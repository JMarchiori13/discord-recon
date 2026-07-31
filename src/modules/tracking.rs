//! Invite analytics: a local tracking store for invite snapshots.
//!
//! Every `invite --track` lookup appends one record to a JSON store at
//! `~/.discord-recon/tracking.json` (or `$DISCORD_RECON_DATA_DIR` if set).
//! All data stays local; no Discord account is needed and each check is a
//! single polite request to the public invites endpoint.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::output::ModuleOutput;

/// One recorded invite snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackRecord {
    pub timestamp: String,
    pub invite_code: String,
    pub guild_id: String,
    pub guild_name: String,
    pub approx_members: u64,
    pub approx_online: u64,
}

/// The store on disk: invite code -> chronologically ordered records.
type Store = BTreeMap<String, Vec<TrackRecord>>;

/// Store path: `$DISCORD_RECON_DATA_DIR/tracking.json` or
/// `~/.discord-recon/tracking.json`.
pub fn store_path() -> Result<PathBuf> {
    if let Ok(dir) = env::var("DISCORD_RECON_DATA_DIR") {
        return Ok(PathBuf::from(dir).join("tracking.json"));
    }
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .context("cannot locate home directory (set DISCORD_RECON_DATA_DIR)")?;
    Ok(PathBuf::from(home)
        .join(".discord-recon")
        .join("tracking.json"))
}

/// Load the whole store from `path` (missing file = empty store).
pub fn load_store(path: &Path) -> Result<Store> {
    if !path.exists() {
        return Ok(Store::new());
    }
    let content =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let store: Store =
        serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    Ok(store)
}

/// Persist the store to `path`, creating parent directories.
fn save_store(path: &Path, store: &Store) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(store)?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Record one invite snapshot in the store at `path`.
pub fn record(path: &Path, rec: TrackRecord) -> Result<()> {
    let mut store = load_store(path)?;
    store.entry(rec.invite_code.clone()).or_default().push(rec);
    save_store(path, &store)
}

/// Load the recorded history for one invite code.
pub fn load_history(path: &Path, code: &str) -> Result<Vec<TrackRecord>> {
    let store = load_store(path)?;
    Ok(store.get(code).cloned().unwrap_or_default())
}

/// One delta between two consecutive records.
#[derive(Debug, PartialEq)]
pub struct Delta {
    pub members_abs: i64,
    pub members_pct: f64,
    pub online_abs: i64,
    pub online_pct: f64,
}

/// Percent change from `prev` to `next` (0.0 when prev is 0).
fn pct_change(prev: u64, next: u64) -> f64 {
    if prev == 0 {
        return 0.0;
    }
    ((next as f64 - prev as f64) / prev as f64) * 100.0
}

/// Delta between two records.
pub fn delta(prev: &TrackRecord, next: &TrackRecord) -> Delta {
    Delta {
        members_abs: next.approx_members as i64 - prev.approx_members as i64,
        members_pct: pct_change(prev.approx_members, next.approx_members),
        online_abs: next.approx_online as i64 - prev.approx_online as i64,
        online_pct: pct_change(prev.approx_online, next.approx_online),
    }
}

/// Format a delta for the console table: `▲ +120 (+4.2%)`, `▼ -30 (-1.1%)`,
/// `= 0 (0.0%)`. The table printer colors ▲ green and ▼ red.
pub fn format_delta(abs: i64, pct: f64) -> String {
    if abs > 0 {
        format!("▲ +{abs} (+{pct:.1}%)")
    } else if abs < 0 {
        format!("▼ {abs} ({pct:.1}%)")
    } else {
        format!("= 0 ({pct:.1}%)")
    }
}

/// Build the `history` output for one invite code (network-free).
pub fn history_output(path: &Path, code: &str) -> Result<ModuleOutput> {
    let records = load_history(path, code)?;

    if records.is_empty() {
        return Ok(ModuleOutput {
            name: "Invite history",
            json: serde_json::json!({
                "module": "history",
                "invite_code": code,
                "records": [],
                "note": "no recorded snapshots; run `invite --track` first",
            }),
            headers: vec!["Timestamp", "Members", "Online", "Δ Members", "Δ Online"],
            rows: vec![vec![
                "(no records yet — run `discord-recon invite <code> --track` to start tracking)"
                    .to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ]],
        });
    }

    let first = &records[0];
    let last = &records[records.len() - 1];
    let overall = delta(first, last);

    let mut rows: Vec<Vec<String>> = Vec::new();
    for (i, rec) in records.iter().enumerate() {
        let (dm, doln) = if i == 0 {
            ("(first seen)".to_string(), "(first seen)".to_string())
        } else {
            let d = delta(&records[i - 1], rec);
            (
                format_delta(d.members_abs, d.members_pct),
                format_delta(d.online_abs, d.online_pct),
            )
        };
        rows.push(vec![
            rec.timestamp.clone(),
            rec.approx_members.to_string(),
            rec.approx_online.to_string(),
            dm,
            doln,
        ]);
    }

    // Summary row across the whole series.
    rows.push(vec![
        "OVERALL".to_string(),
        format!("{} → {}", first.approx_members, last.approx_members),
        format!("{} → {}", first.approx_online, last.approx_online),
        format_delta(overall.members_abs, overall.members_pct),
        format_delta(overall.online_abs, overall.online_pct),
    ]);

    Ok(ModuleOutput {
        name: "Invite history",
        json: serde_json::json!({
            "module": "history",
            "invite_code": code,
            "guild_id": last.guild_id,
            "guild_name": last.guild_name,
            "first_seen": first.timestamp,
            "last_seen": last.timestamp,
            "snapshots": records.len(),
            "overall": {
                "members_abs": overall.members_abs,
                "members_pct": overall.members_pct,
                "online_abs": overall.online_abs,
                "online_pct": overall.online_pct,
            },
            "records": records,
            "note": "All data is stored locally; each snapshot is one polite request to the public invites endpoint.",
        }),
        headers: vec!["Timestamp", "Members", "Online", "Δ Members", "Δ Online"],
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(code: &str, ts: &str, members: u64, online: u64) -> TrackRecord {
        TrackRecord {
            timestamp: ts.to_string(),
            invite_code: code.to_string(),
            guild_id: "613425648685547541".to_string(),
            guild_name: "Discord Developers".to_string(),
            approx_members: members,
            approx_online: online,
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("discord-recon-test-{name}.json"))
    }

    #[test]
    fn store_roundtrip() {
        let path = temp_path("roundtrip");
        let _ = std::fs::remove_file(&path);

        record(&path, rec("abc", "2026-07-01T00:00:00Z", 100, 10)).unwrap();
        record(&path, rec("abc", "2026-07-02T00:00:00Z", 120, 12)).unwrap();
        record(&path, rec("xyz", "2026-07-01T00:00:00Z", 5, 1)).unwrap();

        let hist = load_history(&path, "abc").unwrap();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0].approx_members, 100);
        assert_eq!(hist[1].approx_members, 120);
        assert_eq!(load_history(&path, "xyz").unwrap().len(), 1);
        assert!(load_history(&path, "missing").unwrap().is_empty());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn delta_math() {
        let a = rec("abc", "t1", 100, 50);
        let b = rec("abc", "t2", 110, 40);
        let d = delta(&a, &b);
        assert_eq!(d.members_abs, 10);
        assert!((d.members_pct - 10.0).abs() < 0.001);
        assert_eq!(d.online_abs, -10);
        assert!((d.online_pct - (-20.0)).abs() < 0.001);

        // Zero-growth and zero-base edge cases.
        let d0 = delta(&a, &a);
        assert_eq!(d0.members_abs, 0);
        assert_eq!(d0.members_pct, 0.0);
        let z = rec("abc", "t0", 0, 0);
        assert_eq!(delta(&z, &a).members_pct, 0.0);
    }

    #[test]
    fn delta_formatting() {
        assert_eq!(format_delta(120, 4.2), "▲ +120 (+4.2%)");
        assert_eq!(format_delta(-30, -1.1), "▼ -30 (-1.1%)");
        assert_eq!(format_delta(0, 0.0), "= 0 (0.0%)");
    }

    #[test]
    fn history_handles_empty_and_series() {
        let path = temp_path("history");
        let _ = std::fs::remove_file(&path);

        let out = history_output(&path, "abc").unwrap();
        assert!(out.rows[0][0].contains("no records"));

        record(&path, rec("abc", "2026-07-01T00:00:00Z", 100, 10)).unwrap();
        record(&path, rec("abc", "2026-07-03T00:00:00Z", 90, 15)).unwrap();
        let out = history_output(&path, "abc").unwrap();
        assert_eq!(out.rows.len(), 3); // 2 points + OVERALL
        assert!(out.rows[1][3].starts_with('▼'));
        assert!(out.rows[1][4].starts_with('▲'));
        assert_eq!(out.rows[2][0], "OVERALL");

        std::fs::remove_file(&path).ok();
    }
}
