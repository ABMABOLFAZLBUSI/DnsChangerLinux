//! NetworkManager integration via `nmcli`.

use anyhow::{anyhow, bail, Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Connection {
    pub name: String,
    pub uuid: String,
    pub device: Option<String>,
    pub conn_type: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DnsSnapshot {
    pub ipv4_dns: Vec<String>,
    pub ipv6_dns: Vec<String>,
    pub ipv4_ignore_auto: bool,
    pub ipv6_ignore_auto: bool,
}

fn nmcli(args: &[&str]) -> Result<String> {
    let output = Command::new("nmcli")
        .args(args)
        .output()
        .context("Failed to run nmcli. Is NetworkManager installed? (pacman -S networkmanager)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("nmcli exited with {}", output.status)
        };
        bail!("{msg}");
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn check_nmcli() -> Result<()> {
    let status = Command::new("nmcli")
        .arg("general")
        .arg("status")
        .output()
        .context("nmcli not found. Install NetworkManager: pacman -S networkmanager")?;
    if !status.status.success() {
        bail!("NetworkManager does not appear to be running");
    }
    Ok(())
}

/// List connections, preferring active ones first.
pub fn list_connections() -> Result<Vec<Connection>> {
    let out = nmcli(&[
        "-t",
        "-f",
        "NAME,UUID,DEVICE,TYPE,ACTIVE",
        "connection",
        "show",
    ])?;

    let mut connections = Vec::new();
    for line in out.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 5 {
            continue;
        }
        let name = parts[0].replace("\\:", ":");
        let uuid = parts[1].to_string();
        let device = {
            let d = parts[2];
            if d.is_empty() || d == "--" {
                None
            } else {
                Some(d.to_string())
            }
        };
        let conn_type = parts[3].to_string();
        let active = parts[4].eq_ignore_ascii_case("yes");

        // Skip loopback / virtual noise when possible
        if conn_type == "loopback" {
            continue;
        }

        connections.push(Connection {
            name,
            uuid,
            device,
            conn_type,
            active,
        });
    }

    connections.sort_by(|a, b| b.active.cmp(&a.active).then(a.name.cmp(&b.name)));
    Ok(connections)
}

pub fn get_connection_dns(conn_name_or_uuid: &str) -> Result<DnsSnapshot> {
    let out = nmcli(&[
        "-t",
        "-f",
        "ipv4.dns,ipv4.ignore-auto-dns,ipv6.dns,ipv6.ignore-auto-dns",
        "connection",
        "show",
        conn_name_or_uuid,
    ])?;

    let mut snap = DnsSnapshot::default();
    for line in out.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key {
            "ipv4.dns" => {
                snap.ipv4_dns = split_dns_list(value);
            }
            "ipv6.dns" => {
                snap.ipv6_dns = split_dns_list(value);
            }
            "ipv4.ignore-auto-dns" => {
                snap.ipv4_ignore_auto = value.eq_ignore_ascii_case("yes");
            }
            "ipv6.ignore-auto-dns" => {
                snap.ipv6_ignore_auto = value.eq_ignore_ascii_case("yes");
            }
            _ => {}
        }
    }
    Ok(snap)
}

/// Effective DNS as seen on the device (includes DHCP when not ignored).
pub fn get_device_dns(device: &str) -> Result<(Vec<String>, Vec<String>)> {
    let out = nmcli(&["-t", "-f", "IP4.DNS,IP6.DNS", "device", "show", device])?;
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    for line in out.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if key.starts_with("IP4.DNS") && !value.is_empty() {
            v4.push(value.to_string());
        } else if key.starts_with("IP6.DNS") && !value.is_empty() {
            v6.push(value.to_string());
        }
    }
    Ok((v4, v6))
}

fn split_dns_list(value: &str) -> Vec<String> {
    value
        .split(|c| c == ' ' || c == ',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn apply_dns(
    conn_name_or_uuid: &str,
    ipv4: &[String],
    ipv6: &[String],
    use_ipv6: bool,
) -> Result<()> {
    if ipv4.is_empty() && (!use_ipv6 || ipv6.is_empty()) {
        bail!("At least one DNS server address is required");
    }

    let ipv4_joined = ipv4.join(" ");
    nmcli(&[
        "connection",
        "modify",
        conn_name_or_uuid,
        "ipv4.dns",
        &ipv4_joined,
        "ipv4.ignore-auto-dns",
        "yes",
    ])?;

    if use_ipv6 {
        let ipv6_joined = ipv6.join(" ");
        nmcli(&[
            "connection",
            "modify",
            conn_name_or_uuid,
            "ipv6.dns",
            &ipv6_joined,
            "ipv6.ignore-auto-dns",
            if ipv6.is_empty() { "no" } else { "yes" },
        ])?;
    }

    // Bring connection up to apply (may prompt polkit)
    nmcli(&["connection", "up", conn_name_or_uuid])
        .context("Failed to activate connection after DNS change (check polkit permissions)")?;

    Ok(())
}

pub fn restore_auto_dns(conn_name_or_uuid: &str) -> Result<()> {
    nmcli(&[
        "connection",
        "modify",
        conn_name_or_uuid,
        "ipv4.dns",
        "",
        "ipv4.ignore-auto-dns",
        "no",
        "ipv6.dns",
        "",
        "ipv6.ignore-auto-dns",
        "no",
    ])?;
    nmcli(&["connection", "up", conn_name_or_uuid])?;
    Ok(())
}

pub fn apply_snapshot(conn_name_or_uuid: &str, snap: &DnsSnapshot) -> Result<()> {
    let ipv4 = snap.ipv4_dns.join(" ");
    let ipv6 = snap.ipv6_dns.join(" ");
    nmcli(&[
        "connection",
        "modify",
        conn_name_or_uuid,
        "ipv4.dns",
        &ipv4,
        "ipv4.ignore-auto-dns",
        if snap.ipv4_ignore_auto { "yes" } else { "no" },
        "ipv6.dns",
        &ipv6,
        "ipv6.ignore-auto-dns",
        if snap.ipv6_ignore_auto { "yes" } else { "no" },
    ])?;
    nmcli(&["connection", "up", conn_name_or_uuid])?;
    Ok(())
}

pub fn format_dns_summary(v4: &[String], v6: &[String]) -> String {
    let mut parts = Vec::new();
    if !v4.is_empty() {
        parts.push(format!("IPv4: {}", v4.join(", ")));
    }
    if !v6.is_empty() {
        parts.push(format!("IPv6: {}", v6.join(", ")));
    }
    if parts.is_empty() {
        "No DNS configured / using automatic".into()
    } else {
        parts.join(" · ")
    }
}

#[allow(dead_code)]
pub fn active_connection_id(connections: &[Connection]) -> Option<String> {
    connections
        .iter()
        .find(|c| c.active)
        .map(|c| c.uuid.clone())
        .or_else(|| connections.first().map(|c| c.uuid.clone()))
}

#[allow(dead_code)]
pub fn connection_by_uuid<'a>(
    connections: &'a [Connection],
    uuid: &str,
) -> Result<&'a Connection> {
    connections
        .iter()
        .find(|c| c.uuid == uuid)
        .ok_or_else(|| anyhow!("Connection not found"))
}
