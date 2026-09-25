//! DNS cache flush and resolve-latency benchmarking.

use anyhow::{bail, Context, Result};
use hickory_resolver::config::{NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::name_server::TokioConnectionProvider;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::proto::xfer::Protocol;
use hickory_resolver::Resolver;
use std::net::{IpAddr, SocketAddr};
use std::process::Command;
use std::str::FromStr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct BenchResult {
    pub server_id: String,
    #[allow(dead_code)]
    pub address_tested: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

pub fn flush_caches() -> Result<String> {
    let mut notes = Vec::new();

    match Command::new("resolvectl").arg("flush-caches").output() {
        Ok(out) if out.status.success() => {
            notes.push("systemd-resolved cache flushed".to_string());
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if err.is_empty() {
                notes.push("resolvectl flush-caches failed (may need privileges)".into());
            } else {
                notes.push(format!("resolvectl: {err}"));
            }
        }
        Err(_) => notes.push("resolvectl not available".into()),
    }

    // Optional caches
    if Command::new("systemctl")
        .args(["is-active", "--quiet", "nscd"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        let _ = Command::new("nscd").args(["-i", "hosts"]).status();
        notes.push("nscd hosts cache invalidated".into());
    }

    if Command::new("systemctl")
        .args(["is-active", "--quiet", "dnsmasq"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        let _ = Command::new("systemctl").args(["reload", "dnsmasq"]).status();
        notes.push("dnsmasq reloaded".into());
    }

    if notes.is_empty() {
        bail!("No DNS cache service found to flush");
    }
    Ok(notes.join("; "))
}

/// Time an A-record lookup of `hostname` via a specific DNS server address.
pub fn measure_resolve(dns_addr: &str, hostname: &str, timeout_ms: u64) -> Result<u64> {
    let ip = IpAddr::from_str(dns_addr.trim()).context("Invalid DNS IP address")?;
    let socket = SocketAddr::new(ip, 53);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to create tokio runtime")?;

    rt.block_on(async move {
        let mut config = ResolverConfig::new();
        config.add_name_server(NameServerConfig::new(socket, Protocol::Udp));
        config.add_name_server(NameServerConfig::new(socket, Protocol::Tcp));

        let mut opts = ResolverOpts::default();
        opts.timeout = Duration::from_millis(timeout_ms);
        opts.attempts = 1;
        opts.validate = false;

        let resolver =
            Resolver::builder_with_config(config, TokioConnectionProvider::default())
                .with_options(opts)
                .build();

        let start = Instant::now();
        resolver
            .lookup(hostname, RecordType::A)
            .await
            .with_context(|| format!("Lookup of {hostname} via {dns_addr} failed"))?;
        Ok(start.elapsed().as_millis() as u64)
    })
}

pub fn benchmark_servers(
    servers: &[(String, String)],
    hostname: &str,
    timeout_ms: u64,
) -> Vec<BenchResult> {
    let mut results = Vec::with_capacity(servers.len());
    for (id, addr) in servers {
        match measure_resolve(addr, hostname, timeout_ms) {
            Ok(ms) => results.push(BenchResult {
                server_id: id.clone(),
                address_tested: addr.clone(),
                latency_ms: Some(ms),
                error: None,
            }),
            Err(e) => results.push(BenchResult {
                server_id: id.clone(),
                address_tested: addr.clone(),
                latency_ms: None,
                error: Some(e.to_string()),
            }),
        }
    }
    results.sort_by(|a, b| match (a.latency_ms, b.latency_ms) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.server_id.cmp(&b.server_id),
    });
    results
}
