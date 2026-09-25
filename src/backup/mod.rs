//! Backup and restore of NetworkManager DNS settings.

use crate::nm::DnsSnapshot;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::presets::config_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsBackup {
    pub connection_uuid: String,
    pub connection_name: String,
    pub snapshot: BackupSnapshot,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSnapshot {
    pub ipv4_dns: Vec<String>,
    pub ipv6_dns: Vec<String>,
    pub ipv4_ignore_auto: bool,
    pub ipv6_ignore_auto: bool,
}

impl From<&DnsSnapshot> for BackupSnapshot {
    fn from(s: &DnsSnapshot) -> Self {
        Self {
            ipv4_dns: s.ipv4_dns.clone(),
            ipv6_dns: s.ipv6_dns.clone(),
            ipv4_ignore_auto: s.ipv4_ignore_auto,
            ipv6_ignore_auto: s.ipv6_ignore_auto,
        }
    }
}

impl From<&BackupSnapshot> for DnsSnapshot {
    fn from(s: &BackupSnapshot) -> Self {
        Self {
            ipv4_dns: s.ipv4_dns.clone(),
            ipv6_dns: s.ipv6_dns.clone(),
            ipv4_ignore_auto: s.ipv4_ignore_auto,
            ipv6_ignore_auto: s.ipv6_ignore_auto,
        }
    }
}

fn backup_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("backup.json"))
}

pub fn load_backup() -> Result<Option<DnsBackup>> {
    let path = backup_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)?;
    let backup: DnsBackup = serde_json::from_str(&data)?;
    Ok(Some(backup))
}

pub fn save_backup(backup: &DnsBackup) -> Result<()> {
    let path = backup_path()?;
    let data = serde_json::to_string_pretty(backup)?;
    fs::write(path, data)?;
    Ok(())
}

/// Save a backup only if none exists yet (first-apply auto-backup).
pub fn auto_backup_if_missing(
    connection_uuid: &str,
    connection_name: &str,
    snapshot: &DnsSnapshot,
) -> Result<bool> {
    if load_backup()?.is_some() {
        return Ok(false);
    }
    let backup = DnsBackup {
        connection_uuid: connection_uuid.to_string(),
        connection_name: connection_name.to_string(),
        snapshot: BackupSnapshot::from(snapshot),
        created_at: chrono_like_now(),
    };
    save_backup(&backup)?;
    Ok(true)
}

pub fn force_backup(
    connection_uuid: &str,
    connection_name: &str,
    snapshot: &DnsSnapshot,
) -> Result<()> {
    let backup = DnsBackup {
        connection_uuid: connection_uuid.to_string(),
        connection_name: connection_name.to_string(),
        snapshot: BackupSnapshot::from(snapshot),
        created_at: chrono_like_now(),
    };
    save_backup(&backup)
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

pub fn restore_from_backup() -> Result<(String, DnsSnapshot)> {
    let backup = load_backup()?.context("No backup found")?;
    Ok((
        backup.connection_uuid.clone(),
        DnsSnapshot::from(&backup.snapshot),
    ))
}
