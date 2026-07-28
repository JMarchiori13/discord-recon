//! Snowflake decoder — no network access.
//!
//! Discord snowflakes are 64-bit IDs encoding a millisecond timestamp since
//! the Discord epoch (2015-01-01T00:00:00Z) plus worker/process/increment
//! bits. Decoding reveals when an account, guild, channel or message was
//! created — a classic attribution/timeline data point (ATT&CK T1589/T1593).

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use serde_json::json;

use crate::output::ModuleOutput;

/// Discord epoch: 2015-01-01T00:00:00.000Z in Unix milliseconds.
pub const DISCORD_EPOCH: u64 = 1_420_070_400_000;

/// Decoded components of a Discord snowflake.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct SnowflakeInfo {
    pub id: u64,
    pub timestamp_ms: u64,
    pub worker_id: u64,
    pub process_id: u64,
    pub increment: u64,
}

/// Validate that a string is a plausible snowflake (numeric, 64-bit).
pub fn parse_snowflake(raw: &str) -> Result<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        bail!("invalid snowflake {raw:?}: expected a numeric Discord ID");
    }
    let id: u64 = trimmed
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid snowflake {raw:?}: does not fit in 64 bits"))?;
    if id < (1 << 22) {
        bail!("invalid snowflake {raw:?}: too small to encode a timestamp");
    }
    Ok(id)
}

/// Decode a snowflake into its components.
pub fn decode(id: u64) -> SnowflakeInfo {
    SnowflakeInfo {
        id,
        timestamp_ms: (id >> 22) + DISCORD_EPOCH,
        worker_id: (id & 0x3E_0000) >> 17,
        process_id: (id & 0x1F_000) >> 12,
        increment: id & 0xFFF,
    }
}

/// Format milliseconds-since-epoch as `YYYY-MM-DD HH:MM:SS` UTC.
pub fn format_utc(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    DateTime::from_timestamp(secs, 0)
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "(out of range)".to_string())
}

/// Human-readable age from `ms` until now (e.g. "11 years ago").
pub fn relative_age(now: DateTime<Utc>, ms: u64) -> String {
    let then_ms = ms as i64;
    let delta = Duration::milliseconds(now.timestamp_millis() - then_ms);
    let (value, unit) = if delta.num_days() >= 365 {
        (delta.num_days() / 365, "year")
    } else if delta.num_days() >= 30 {
        (delta.num_days() / 30, "month")
    } else if delta.num_days() >= 1 {
        (delta.num_days(), "day")
    } else if delta.num_hours() >= 1 {
        (delta.num_hours(), "hour")
    } else {
        (delta.num_minutes().max(0), "minute")
    };
    let plural = if value == 1 { "" } else { "s" };
    if delta.num_milliseconds() >= 0 {
        format!("{value} {unit}{plural} ago")
    } else {
        format!("in {} {unit}{plural}", value.abs())
    }
}

/// Decode a batch of snowflake IDs (network-free).
pub fn run(ids: &[String]) -> Result<ModuleOutput> {
    let now = Utc::now();
    let mut rows = Vec::new();
    let mut decoded = Vec::new();

    for raw in ids {
        let id = parse_snowflake(raw)?;
        let info = decode(id);
        rows.push(vec![
            id.to_string(),
            format_utc(info.timestamp_ms),
            relative_age(now, info.timestamp_ms),
            info.worker_id.to_string(),
            info.process_id.to_string(),
            info.increment.to_string(),
        ]);
        decoded.push(json!({
            "id": id.to_string(),
            "utc_timestamp": format_utc(info.timestamp_ms),
            "age": relative_age(now, info.timestamp_ms),
            "worker_id": info.worker_id,
            "process_id": info.process_id,
            "increment": info.increment,
        }));
    }

    Ok(ModuleOutput {
        name: "Snowflake decoding",
        json: json!({
            "module": "snowflake",
            "discord_epoch": DISCORD_EPOCH,
            "decoded": decoded,
        }),
        headers: vec![
            "ID",
            "UTC Timestamp",
            "Age",
            "Worker",
            "Process",
            "Increment",
        ],
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn epoch_is_2015_01_01() {
        assert_eq!(DISCORD_EPOCH, 1_420_070_400_000);
        assert_eq!(format_utc(DISCORD_EPOCH), "2015-01-01 00:00:00");
    }

    #[test]
    fn decodes_discord_docs_example() {
        // Example snowflake from Discord's own documentation.
        let info = decode(80351110224678912);
        assert_eq!(info.timestamp_ms, 1_439_227_597_529);
        assert_eq!(format_utc(info.timestamp_ms), "2015-08-10 17:26:37");
        assert_eq!(info.worker_id, 0);
        assert_eq!(info.process_id, 1);
        assert_eq!(info.increment, 0);
    }

    #[test]
    fn decodes_worker_process_increment_bits() {
        // Craft an id with known bits: ts=0, worker=1, process=2, inc=42.
        let id: u64 = (1 << 17) | (2 << 12) | 42;
        let info = decode(id + (1 << 22));
        assert_eq!(info.worker_id, 1);
        assert_eq!(info.process_id, 2);
        assert_eq!(info.increment, 42);
    }

    #[test]
    fn parses_and_rejects_inputs() {
        assert_eq!(
            parse_snowflake("80351110224678912").unwrap(),
            80351110224678912
        );
        assert!(parse_snowflake("abc").is_err());
        assert!(parse_snowflake("").is_err());
        assert!(parse_snowflake("123").is_err()); // too small
        assert!(parse_snowflake("99999999999999999999999999").is_err()); // > u64
    }

    #[test]
    fn computes_relative_age() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let one_year_ms = (now.timestamp_millis() - 365 * 86_400_000) as u64;
        assert_eq!(relative_age(now, one_year_ms), "1 year ago");
        let eleven_years_ms = (now.timestamp_millis() - 11 * 365 * 86_400_000) as u64;
        assert_eq!(relative_age(now, eleven_years_ms), "11 years ago");
    }
}
