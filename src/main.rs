//! discord-recon — a Discord OSINT reconnaissance CLI in Rust.
//!
//! **Read-only, for authorized security research only.** Only public or
//! authorized API surfaces are used: keyless public endpoints (invites,
//! guild widgets, webhook metadata) and official bot-token endpoints (user
//! profiles). NO selfbots, NO user-token automation, NO token checking,
//! NO message scraping, NO mass actions — these violate Discord ToS.

mod http;
mod modules;
mod output;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use output::ModuleOutput;
use serde_json::json;

/// Discord OSINT reconnaissance CLI — read-only, authorized use only.
#[derive(Parser)]
#[command(
    name = "discord-recon",
    version,
    about = "Discord OSINT reconnaissance CLI — read-only, public/authorized API surfaces only",
    long_about = "discord-recon performs READ-ONLY reconnaissance against public or authorized \
                  Discord API surfaces. No selfbots, no user-token automation, no message \
                  scraping, no mass actions. For authorized security research only."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Politeness rate limit in requests per second.
    #[arg(long, global = true, default_value_t = 1.0)]
    rate: f64,

    /// Per-request timeout in seconds.
    #[arg(long, global = true, default_value_t = 15)]
    timeout: u64,

    /// Number of retries per request after the initial attempt.
    #[arg(long, global = true, default_value_t = 2)]
    retries: usize,

    /// Export results as JSON to this path.
    #[arg(long, global = true, value_name = "FILE")]
    json: Option<PathBuf>,

    /// Export results as CSV to this path.
    #[arg(long, global = true, value_name = "FILE")]
    csv: Option<PathBuf>,

    /// Emit results as JSONL on stdout (one JSON object per result) for
    /// composability with jq. Banner and logs go to stderr.
    #[arg(long, global = true)]
    stdout: bool,

    /// Suppress the authorization banner.
    #[arg(short, long, global = true)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Invite intelligence (KEYLESS): guild, counts, features from an invite code/URL.
    Invite {
        /// Invite code or URL (e.g. discord.gg/python, or '-' for stdin).
        target: String,
    },
    /// Guild widget intelligence (KEYLESS when the server widget is enabled).
    Guild {
        /// Numeric guild ID (e.g. 80351110224678912, or '-' for stdin).
        id: String,
    },
    /// Decode Discord snowflakes (KEYLESS, no network): timestamps and bits.
    Snowflake {
        /// One or more snowflake IDs (or '-' to read from stdin).
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Webhook metadata (KEYLESS, read-only — never posts to the webhook).
    Webhook {
        /// Webhook URL or id/token pair (or '-' for stdin).
        target: String,
    },
    /// User profile intelligence (BOT-TOKEN tier: DISCORD_RECON_BOT_TOKEN).
    User {
        /// Numeric user ID (or '-' for stdin).
        id: String,
    },
    /// Smart orchestrator: detect target type and chain applicable modules.
    Full {
        /// Invite code/URL, snowflake ID, or webhook URL (or '-' for stdin).
        target: String,
    },
}

/// Print the mandatory authorization banner (stderr in `--stdout` mode).
fn banner(to_stderr: bool) {
    let lines = [
        "discord-recon — Discord OSINT reconnaissance CLI"
            .bold()
            .cyan()
            .to_string(),
        "Read-only reconnaissance. For authorized security research only."
            .bold()
            .yellow()
            .to_string(),
        "No selfbots, no user-token automation, no scraping, no mass actions."
            .dimmed()
            .to_string(),
    ];
    for line in lines {
        if to_stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    }
}

/// Resolve the target argument: a literal value, or `-` to read one target
/// per line from stdin (blank lines and `#` comments are skipped).
fn resolve_targets(arg: &str) -> Result<Vec<String>> {
    if arg != "-" {
        return Ok(vec![arg.to_string()]);
    }
    use std::io::Read as _;
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .context("reading targets from stdin")?;
    let targets: Vec<String> = buf
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    if targets.is_empty() {
        anyhow::bail!("no targets on stdin (expected one per line)");
    }
    Ok(targets)
}

/// Run a network module over resolved targets, collecting per-target
/// failures as warnings instead of aborting the batch.
fn run_batch<F>(targets: &[String], mut f: F) -> Vec<ModuleOutput>
where
    F: FnMut(&str) -> Result<ModuleOutput>,
{
    let mut outputs = Vec::new();
    for t in targets {
        match f(t) {
            Ok(out) => outputs.push(out),
            Err(e) => {
                if targets.len() == 1 {
                    // Single target: propagate the error for a clear exit code.
                    eprintln!("{} {e:#}", "[error]".red());
                    std::process::exit(1);
                }
                eprintln!("{} target {t:?} failed: {e:#}", "[warn]".yellow());
            }
        }
    }
    outputs
}

/// Render results (table or JSONL) and export files.
fn report(cli: &Cli, outputs: &[ModuleOutput]) -> Result<()> {
    if cli.stdout {
        for out in outputs {
            output::print_jsonl(out);
        }
    } else {
        for out in outputs {
            output::print_table(out.name, &out.headers, &out.rows);
        }
    }

    if let Some(path) = &cli.json {
        let bundle = if outputs.len() == 1 {
            outputs[0].json.clone()
        } else {
            json!({
                "tool": "discord-recon",
                "version": env!("CARGO_PKG_VERSION"),
                "modules": outputs.iter().map(|o| &o.json).collect::<Vec<_>>(),
            })
        };
        output::write_json(path, &bundle)?;
        eprintln!(
            "{} JSON results written to {}",
            "[ok]".green(),
            path.display()
        );
    }

    if let Some(path) = &cli.csv {
        let same_headers = outputs.windows(2).all(|w| w[0].headers == w[1].headers);
        if outputs.len() > 1 && !same_headers {
            eprintln!(
                "{} CSV export needs matching table shapes; use per-module runs or --json",
                "[warn]".yellow()
            );
        } else {
            let (headers, rows) = if outputs.len() > 1 {
                let mut headers = vec!["module"];
                headers.extend_from_slice(&outputs[0].headers);
                let mut rows = Vec::new();
                for out in outputs {
                    let module = out
                        .json
                        .get("module")
                        .and_then(|v| v.as_str())
                        .unwrap_or(out.name)
                        .to_string();
                    for row in &out.rows {
                        let mut r = vec![module.clone()];
                        r.extend(row.iter().cloned());
                        rows.push(r);
                    }
                }
                (headers, rows)
            } else {
                (outputs[0].headers.clone(), outputs[0].rows.clone())
            };
            output::write_csv(path, &headers, &rows)?;
            eprintln!(
                "{} CSV results written to {}",
                "[ok]".green(),
                path.display()
            );
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if !cli.quiet {
        banner(cli.stdout);
    }

    let client = http::HttpClient::new(cli.timeout, cli.rate, cli.retries)
        .context("initializing HTTP client")?;

    let outputs: Vec<ModuleOutput> = match &cli.command {
        Commands::Invite { target } => {
            let targets = resolve_targets(target)?;
            run_batch(&targets, |t| modules::invite::run(&client, t))
        }
        Commands::Guild { id } => {
            let targets = resolve_targets(id)?;
            run_batch(&targets, |t| modules::guild::run(&client, t))
        }
        Commands::Snowflake { ids } => {
            let ids = if ids.len() == 1 && ids[0] == "-" {
                resolve_targets("-")?
            } else {
                ids.clone()
            };
            vec![modules::snowflake::run(&ids)?]
        }
        Commands::Webhook { target } => {
            let targets = resolve_targets(target)?;
            run_batch(&targets, |t| modules::webhook::run(&client, t))
        }
        Commands::User { id } => {
            let targets = resolve_targets(id)?;
            run_batch(&targets, |t| modules::user::run(&client, t))
        }
        Commands::Full { target } => {
            let targets = resolve_targets(target)?;
            targets
                .iter()
                .flat_map(|t| modules::full::run(&client, t))
                .collect()
        }
    };

    report(&cli, &outputs)
}
