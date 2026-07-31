//! Integration tests for the discord-recon CLI (network-free).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_discord-recon"))
}

#[test]
fn help_lists_all_subcommands() {
    let out = bin().arg("--help").output().expect("run --help");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for sub in [
        "invite",
        "guild",
        "snowflake",
        "webhook",
        "user",
        "full",
        "history",
    ] {
        assert!(stdout.contains(sub), "missing subcommand {sub} in --help");
    }
}

#[test]
fn banner_states_read_only_authorized_use() {
    let out = bin()
        .args(["snowflake", "abc"])
        .output()
        .expect("run snowflake");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("For authorized security research only."));
    assert!(stdout.contains("No selfbots"));
}

#[test]
fn snowflake_decodes_offline() {
    let out = bin()
        .args(["--quiet", "snowflake", "80351110224678912"])
        .output()
        .expect("run snowflake");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("2015-08-10 17:26:37"));
    assert!(stdout.contains("80351110224678912"));
}

#[test]
fn snowflake_jsonl_mode() {
    let out = bin()
        .args(["--stdout", "snowflake", "80351110224678912"])
        .output()
        .expect("run snowflake --stdout");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // stdout is pure JSONL; banner moved to stderr.
    let line: serde_json::Value =
        serde_json::from_str(stdout.lines().next().unwrap()).expect("valid JSONL");
    assert_eq!(line["module"], "snowflake");
    assert_eq!(line["id"], "80351110224678912");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("For authorized security research only."));
}

#[test]
fn snowflake_rejects_invalid_ids() {
    let out = bin()
        .args(["--quiet", "snowflake", "not-a-snowflake"])
        .output()
        .expect("run snowflake");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("invalid snowflake"));
}

#[test]
fn user_without_token_explains_tier() {
    // Ensure the env var is absent for this test.
    let out = bin()
        .args(["--quiet", "user", "80351110224678912"])
        .env_remove("DISCORD_RECON_BOT_TOKEN")
        .output()
        .expect("run user");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DISCORD_RECON_BOT_TOKEN"));
    assert!(stderr.contains("bot-token tier"));
}

#[test]
fn stdin_dash_with_empty_input_errors_cleanly() {
    let out = bin()
        .args(["--quiet", "snowflake", "-"])
        .output() // stdin is null → immediate EOF
        .expect("run snowflake -");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no targets on stdin"));
}
