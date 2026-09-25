//! Built-in presets, custom servers, and user groups.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsServer {
    pub id: String,
    pub name: String,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    #[serde(default)]
    pub builtin: bool,
    #[serde(default)]
    pub group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub groups: Vec<DnsGroup>,
    pub servers: Vec<DnsServer>,
    #[serde(default = "default_test_domain")]
    pub test_domain: String,
    #[serde(default = "default_true")]
    pub use_ipv6: bool,
    #[serde(default)]
    pub last_connection: Option<String>,
    #[serde(default)]
    pub selected_server_id: Option<String>,
}

fn default_test_domain() -> String {
    "www.google.com".into()
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            groups: default_groups(),
            servers: builtin_servers(),
            test_domain: default_test_domain(),
            use_ipv6: true,
            last_connection: None,
            selected_server_id: None,
        }
    }
}

fn default_groups() -> Vec<DnsGroup> {
    vec![
        DnsGroup {
            id: "public".into(),
            name: "Public".into(),
        },
        DnsGroup {
            id: "privacy".into(),
            name: "Privacy".into(),
        },
        DnsGroup {
            id: "custom".into(),
            name: "Custom".into(),
        },
    ]
}

pub fn builtin_servers() -> Vec<DnsServer> {
    vec![
        DnsServer {
            id: "cloudflare".into(),
            name: "Cloudflare".into(),
            ipv4: vec!["1.1.1.1".into(), "1.0.0.1".into()],
            ipv6: vec!["2606:4700:4700::1111".into(), "2606:4700:4700::1001".into()],
            builtin: true,
            group: "public".into(),
        },
        DnsServer {
            id: "google".into(),
            name: "Google".into(),
            ipv4: vec!["8.8.8.8".into(), "8.8.4.4".into()],
            ipv6: vec!["2001:4860:4860::8888".into(), "2001:4860:4860::8844".into()],
            builtin: true,
            group: "public".into(),
        },
        DnsServer {
            id: "quad9".into(),
            name: "Quad9".into(),
            ipv4: vec!["9.9.9.9".into(), "149.112.112.112".into()],
            ipv6: vec!["2620:fe::fe".into(), "2620:fe::9".into()],
            builtin: true,
            group: "privacy".into(),
        },
        DnsServer {
            id: "adguard".into(),
            name: "AdGuard".into(),
            ipv4: vec!["94.140.14.14".into(), "94.140.15.15".into()],
            ipv6: vec!["2a10:50c0::ad1:ff".into(), "2a10:50c0::ad2:ff".into()],
            builtin: true,
            group: "privacy".into(),
        },
        DnsServer {
            id: "opendns".into(),
            name: "OpenDNS".into(),
            ipv4: vec!["208.67.222.222".into(), "208.67.220.220".into()],
            ipv6: vec!["2620:119:35::35".into(), "2620:119:53::53".into()],
            builtin: true,
            group: "public".into(),
        },
        DnsServer {
            id: "mullvad".into(),
            name: "Mullvad".into(),
            ipv4: vec!["194.242.2.2".into(), "194.242.2.3".into()],
            ipv6: vec!["2a07:e340::2".into(), "2a07:e340::3".into()],
            builtin: true,
            group: "privacy".into(),
        },
    ]
}

pub fn config_dir() -> Result<PathBuf> {
    let base = directories::ProjectDirs::from("io", "github", "DnsJump")
        .context("Could not resolve config directory")?;
    let dir = base.config_dir().to_path_buf();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        let cfg = AppConfig::default();
        save_config(&cfg)?;
        return Ok(cfg);
    }
    let data = fs::read_to_string(&path)?;
    let mut cfg: AppConfig = serde_json::from_str(&data)?;

    // Ensure builtins exist (merge by id)
    for builtin in builtin_servers() {
        if !cfg.servers.iter().any(|s| s.id == builtin.id) {
            cfg.servers.push(builtin);
        }
    }
    for g in default_groups() {
        if !cfg.groups.iter().any(|x| x.id == g.id) {
            cfg.groups.push(g);
        }
    }
    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    let path = config_path()?;
    let data = serde_json::to_string_pretty(cfg)?;
    fs::write(path, data)?;
    Ok(())
}

pub fn add_custom_server(
    cfg: &mut AppConfig,
    name: String,
    ipv4: Vec<String>,
    ipv6: Vec<String>,
    group: Option<String>,
) -> String {
    let id = Uuid::new_v4().to_string();
    cfg.servers.push(DnsServer {
        id: id.clone(),
        name,
        ipv4,
        ipv6,
        builtin: false,
        group: group.unwrap_or_else(|| "custom".into()),
    });
    id
}

pub fn remove_server(cfg: &mut AppConfig, id: &str) -> bool {
    if let Some(pos) = cfg.servers.iter().position(|s| s.id == id && !s.builtin) {
        cfg.servers.remove(pos);
        true
    } else {
        false
    }
}

pub fn add_group(cfg: &mut AppConfig, name: String) -> String {
    let id = Uuid::new_v4().to_string();
    cfg.groups.push(DnsGroup {
        id: id.clone(),
        name,
    });
    id
}

pub fn remove_group(cfg: &mut AppConfig, id: &str) -> bool {
    if id == "public" || id == "privacy" || id == "custom" {
        return false;
    }
    cfg.groups.retain(|g| g.id != id);
    for s in &mut cfg.servers {
        if s.group == id {
            s.group = "custom".into();
        }
    }
    true
}

#[allow(dead_code)]
pub fn servers_in_group<'a>(cfg: &'a AppConfig, group_id: &str) -> Vec<&'a DnsServer> {
    cfg.servers.iter().filter(|s| s.group == group_id).collect()
}

pub fn find_server<'a>(cfg: &'a AppConfig, id: &str) -> Option<&'a DnsServer> {
    cfg.servers.iter().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_ipv4() {
        for s in builtin_servers() {
            assert!(!s.ipv4.is_empty(), "{} missing ipv4", s.name);
        }
    }

    #[test]
    fn config_roundtrip() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.servers.len(), cfg.servers.len());
    }
}
