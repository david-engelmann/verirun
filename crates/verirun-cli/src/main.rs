//! The verirun executable: a thin shell over the library in
//! `lib.rs` (docs/ARCHITECTURE.md section 2.4).
//!
//! This module owns argument parsing, the seed line, output
//! rendering, and the top level error to exit code mapping of
//! section 12.2. The mapping is total: success and verified
//! and equivalent exit 0, task failure and a differing `diff`
//! exit 1, integrity failures exit 2, and invalid input of
//! every kind exits 3. Argument parsing failures are
//! intercepted here and exit 3, never the argument library's
//! default 2, so a usage error can never report as tampering
//! (INV-CLI-1).

#![forbid(unsafe_code)]

use std::path::PathBuf;

use clap::{Arg, ArgAction, Command};
use verirun_cli::{CliError, error_json};
use verirun_core::digest::Digest;
use verirun_core::manifest::TaskDifference;
use verirun_core::paths::NormalizedPath;

fn main() {
    std::process::exit(real_main());
}

/// Builds one single-manifest subcommand (`verify`, `show`,
/// `pin`, `unpin`).
fn digest_subcommand(name: &'static str, about: &'static str, cas: Arg, json: Arg) -> Command {
    Command::new(name)
        .about(about)
        .arg(
            Arg::new("digest")
                .value_name("MANIFEST_DIGEST")
                .required(true),
        )
        .arg(cas)
        .arg(json)
}

/// Builds the `run` subcommand definition.
fn run_subcommand(cas: Arg, json: Arg) -> Command {
    Command::new("run")
        .about("Run a run file and print the manifest digest")
        .arg(
            Arg::new("run_file")
                .value_name("RUN_FILE")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf)),
        )
        .arg(cas)
        .arg(
            Arg::new("work")
                .long("work")
                .value_name("DIR")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf))
                .help("The directory holding task sandboxes"),
        )
        .arg(
            Arg::new("jobs")
                .long("jobs")
                .value_name("N")
                .value_parser(clap::value_parser!(u64))
                .help("Worker pool size; 0 selects the available parallelism"),
        )
        .arg(
            Arg::new("inputs")
                .long("input")
                .value_name("REL_PATH=HOST_FILE")
                .action(ArgAction::Append)
                .help("Bind a sandbox-relative input path to a host file"),
        )
        .arg(
            Arg::new("pin")
                .long("pin")
                .action(ArgAction::SetTrue)
                .help("Pin the produced manifest so garbage collection preserves it"),
        )
        .arg(json)
}

/// Builds the argument parser for the surface of section
/// 12.1. `--seed` is a global argument: every command accepts
/// it and every run prints the active seed.
fn build_cli() -> Command {
    let cas = Arg::new("cas")
        .long("cas")
        .value_name("DIR")
        .required(true)
        .value_parser(clap::value_parser!(PathBuf))
        .help("The content-addressed store directory");
    let json = Arg::new("json")
        .long("json")
        .action(ArgAction::SetTrue)
        .help("Render the result as one JSON object");
    Command::new("verirun")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Content-addressed deterministic execution runner")
        .subcommand_required(true)
        .arg(
            Arg::new("seed")
                .long("seed")
                .value_name("N")
                .global(true)
                .value_parser(clap::value_parser!(u64))
                .help("The active seed, printed to stderr on every run"),
        )
        .subcommand(run_subcommand(cas.clone(), json.clone()))
        .subcommand(digest_subcommand(
            "verify",
            "Verify a manifest and everything it names",
            cas.clone(),
            json.clone(),
        ))
        .subcommand(digest_subcommand(
            "show",
            "Verify a manifest and print its task records",
            cas.clone(),
            json.clone(),
        ))
        .subcommand(
            Command::new("diff")
                .about("Verify two manifests and compare their records")
                .arg(Arg::new("first").value_name("MANIFEST_A").required(true))
                .arg(Arg::new("second").value_name("MANIFEST_B").required(true))
                .arg(cas.clone())
                .arg(json.clone()),
        )
        .subcommand(digest_subcommand(
            "pin",
            "Verify a manifest and pin it against garbage collection",
            cas.clone(),
            json.clone(),
        ))
        .subcommand(digest_subcommand(
            "unpin",
            "Remove a manifest's garbage collection pin",
            cas.clone(),
            json.clone(),
        ))
        .subcommand(
            Command::new("gc")
                .about("Collect unreachable objects older than the grace threshold")
                .arg(cas)
                .arg(
                    Arg::new("grace_secs")
                        .long("grace-secs")
                        .value_name("N")
                        .value_parser(clap::value_parser!(u64))
                        .help("Grace threshold in seconds; default 3600"),
                )
                .arg(
                    Arg::new("dry_run")
                        .long("dry-run")
                        .action(ArgAction::SetTrue)
                        .help("Print the plan and delete nothing"),
                )
                .arg(json),
        )
}

/// The active seed: the `--seed` flag, else the `SIM_SEED`
/// environment variable, else 0 (section 12.1).
fn resolve_seed(matches: &clap::ArgMatches) -> Result<u64, CliError> {
    if let Some(seed) = matches.get_one::<u64>("seed") {
        return Ok(*seed);
    }
    std::env::var("SIM_SEED").map_or(Ok(0), |text| {
        text.parse::<u64>().map_err(|_| CliError::Argument {
            detail: format!("the SIM_SEED value {text:?} is not an unsigned integer"),
        })
    })
}

/// Parses a digest argument. A malformed digest is invalid
/// input and exits 3, like every argument error.
fn parse_digest(text: &str) -> Result<Digest, CliError> {
    Digest::from_hex(text).map_err(|source| CliError::Argument {
        detail: format!("invalid manifest digest {text:?}: {source}"),
    })
}

/// Renders a failure in both forms (INV-CLI-2) and returns
/// its exit code. The JSON form is never the only place a
/// failure is explained: the text form always goes to stderr.
fn fail(error: &CliError, json: bool) -> i32 {
    eprintln!("error: {error}");
    if json {
        println!("{}", error_json(error));
    }
    error.exit_code()
}

fn real_main() -> i32 {
    let matches = match build_cli().try_get_matches() {
        Ok(matches) => matches,
        Err(err) => {
            use clap::error::ErrorKind;
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                print!("{err}");
                return 0;
            }
            let wants_json = std::env::args_os()
                .skip(1)
                .take_while(|arg| arg != "--")
                .any(|arg| arg == "--json");
            if wants_json {
                let cli_err = CliError::Argument {
                    detail: err.to_string().trim().to_owned(),
                };
                return fail(&cli_err, true);
            }
            eprint!("{err}");
            return 3;
        }
    };
    let Some((name, sub)) = matches.subcommand() else {
        return 3;
    };
    let json = sub.get_flag("json");
    let seed = match resolve_seed(&matches) {
        Ok(seed) => seed,
        Err(error) => return fail(&error, json),
    };
    eprintln!("seed: {seed}");
    match name {
        "run" => run_command(sub, json),
        "verify" => verify_command(sub, json),
        "show" => show_command(sub, json),
        "diff" => diff_command(sub, json),
        "pin" => pin_command(sub, json),
        "unpin" => unpin_command(sub, json),
        "gc" => gc_command(sub, json),
        _ => 3,
    }
}

fn run_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let Some(spec) = sub.get_one::<PathBuf>("run_file") else {
        return 3;
    };
    let Some(cas) = sub.get_one::<PathBuf>("cas") else {
        return 3;
    };
    let Some(work) = sub.get_one::<PathBuf>("work") else {
        return 3;
    };
    let jobs = sub.get_one::<u64>("jobs").copied().unwrap_or(0);
    let jobs = usize::try_from(jobs).unwrap_or(usize::MAX);
    let pin = sub.get_flag("pin");
    let mut bindings: Vec<(NormalizedPath, PathBuf)> = Vec::new();
    if let Some(raw_inputs) = sub.get_many::<String>("inputs") {
        for raw in raw_inputs {
            match verirun_cli::cmd_run::parse_input_binding(raw) {
                Ok(pair) => bindings.push(pair),
                Err(error) => return fail(&error, json),
            }
        }
    }
    match verirun_cli::cmd_run::run_with_options(spec, cas, work, &bindings, jobs, pin) {
        Ok(outcome) => {
            for record in &outcome.manifest.tasks {
                if matches!(
                    record.status,
                    verirun_core::digest::TaskStatus::Failed
                        | verirun_core::digest::TaskStatus::TimedOut
                        | verirun_core::digest::TaskStatus::Skipped
                ) {
                    match outcome.failures.get(&record.task_id) {
                        Some(detail) => eprintln!(
                            "task {}: {} ({detail})",
                            record.task_id,
                            record.status.as_str()
                        ),
                        None => {
                            eprintln!("task {}: {}", record.task_id, record.status.as_str());
                        }
                    }
                }
            }
            if json {
                println!("{}", run_json(&outcome, pin));
            } else {
                println!("{}", outcome.manifest_digest.to_hex());
            }
            outcome.exit_code
        }
        Err(error) => fail(&error, json),
    }
}

/// The `--json` form of a run result: the manifest digest,
/// the exit code, whether the manifest was pinned, every
/// task's status, and the failure details, the same
/// information the text form carries.
fn run_json(outcome: &verirun_store::scheduler::RunOutcome, pinned: bool) -> serde_json::Value {
    let tasks: Vec<serde_json::Value> = outcome
        .manifest
        .tasks
        .iter()
        .map(|record| {
            serde_json::json!({
                "task_id": record.task_id,
                "status": record.status.as_str(),
                "exit_code": record.exit_code,
            })
        })
        .collect();
    let failures: Vec<serde_json::Value> = outcome
        .failures
        .iter()
        .map(|(task_id, detail)| serde_json::json!({ "task_id": task_id, "detail": detail }))
        .collect();
    serde_json::json!({
        "manifest": outcome.manifest_digest.to_hex(),
        "exit_code": outcome.exit_code,
        "pinned": pinned,
        "tasks": tasks,
        "failures": failures,
    })
}

fn verify_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let Some(text) = sub.get_one::<String>("digest") else {
        return 3;
    };
    let Some(cas) = sub.get_one::<PathBuf>("cas") else {
        return 3;
    };
    let digest = match parse_digest(text) {
        Ok(digest) => digest,
        Err(error) => return fail(&error, json),
    };
    match verirun_cli::cmd_verify::verify(cas, &digest) {
        Ok(manifest) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "manifest": digest.to_hex(),
                        "verified": true,
                        "merkle_root": manifest.merkle_root.to_hex(),
                    })
                );
            } else {
                println!("verified {}", digest.to_hex());
            }
            0
        }
        Err(error) => fail(&error, json),
    }
}

fn show_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let Some(text) = sub.get_one::<String>("digest") else {
        return 3;
    };
    let Some(cas) = sub.get_one::<PathBuf>("cas") else {
        return 3;
    };
    let digest = match parse_digest(text) {
        Ok(digest) => digest,
        Err(error) => return fail(&error, json),
    };
    match verirun_cli::cmd_show::show(cas, &digest) {
        Ok(manifest) => {
            if json {
                println!("{}", show_json(&digest, &manifest));
            } else {
                println!("manifest {}", digest.to_hex());
                println!("merkle root {}", manifest.merkle_root.to_hex());
                for record in &manifest.tasks {
                    match record.exit_code {
                        Some(code) => println!(
                            "task {}: {}, exit code {code}",
                            record.task_id,
                            record.status.as_str()
                        ),
                        None => println!("task {}: {}", record.task_id, record.status.as_str()),
                    }
                    println!("  exec {}", record.exec_digest.to_hex());
                    println!("  stdout {}", record.stdout_digest.to_hex());
                    println!("  stderr {}", record.stderr_digest.to_hex());
                    for artifact in &record.artifacts {
                        println!(
                            "  artifact {} {} ({} bytes)",
                            artifact.path,
                            artifact.digest.to_hex(),
                            artifact.bytes
                        );
                    }
                }
            }
            0
        }
        Err(error) => fail(&error, json),
    }
}

/// The `--json` form of a show result: the manifest digest,
/// its Merkle root, and every task record with the same
/// fields the text form prints (INV-CLI-2).
fn show_json(digest: &Digest, manifest: &verirun_core::manifest::Manifest) -> serde_json::Value {
    let tasks: Vec<serde_json::Value> = manifest
        .tasks
        .iter()
        .map(|record| {
            let artifacts: Vec<serde_json::Value> = record
                .artifacts
                .iter()
                .map(|artifact| {
                    serde_json::json!({
                        "path": artifact.path,
                        "digest": artifact.digest.to_hex(),
                        "bytes": artifact.bytes,
                        "executable": artifact.executable,
                    })
                })
                .collect();
            serde_json::json!({
                "task_id": record.task_id,
                "status": record.status.as_str(),
                "exit_code": record.exit_code,
                "exec_digest": record.exec_digest.to_hex(),
                "stdout": record.stdout_digest.to_hex(),
                "stderr": record.stderr_digest.to_hex(),
                "artifacts": artifacts,
            })
        })
        .collect();
    serde_json::json!({
        "manifest": digest.to_hex(),
        "merkle_root": manifest.merkle_root.to_hex(),
        "tasks": tasks,
    })
}

fn diff_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let (Some(first_text), Some(second_text), Some(cas)) = (
        sub.get_one::<String>("first"),
        sub.get_one::<String>("second"),
        sub.get_one::<PathBuf>("cas"),
    ) else {
        return 3;
    };
    let first = match parse_digest(first_text) {
        Ok(digest) => digest,
        Err(error) => return fail(&error, json),
    };
    let second = match parse_digest(second_text) {
        Ok(digest) => digest,
        Err(error) => return fail(&error, json),
    };
    match verirun_cli::cmd_diff::diff(cas, &first, &second) {
        Ok(outcome) => {
            if json {
                println!("{}", diff_json(&outcome));
            } else if outcome.differences.is_empty() {
                println!("runs are equivalent");
            } else {
                for diff in &outcome.differences {
                    println!(
                        "task {}: {}",
                        diff.task_id,
                        difference_text(diff.difference)
                    );
                }
            }
            i32::from(!outcome.differences.is_empty())
        }
        Err(error) => fail(&error, json),
    }
}

/// The text form of one task difference (section 9.4).
fn difference_text(difference: TaskDifference) -> String {
    match difference {
        TaskDifference::OnlyInFirst => "only in first".to_string(),
        TaskDifference::OnlyInSecond => "only in second".to_string(),
        TaskDifference::Changed(fields) => {
            let mut parts: Vec<&str> = Vec::new();
            if fields.status {
                parts.push("status");
            }
            if fields.exit_code {
                parts.push("exit code");
            }
            if fields.artifacts {
                parts.push("artifacts");
            }
            if fields.stdout {
                parts.push("stdout");
            }
            if fields.stderr {
                parts.push("stderr");
            }
            format!("changed ({})", parts.join(", "))
        }
    }
}

/// The `--json` form of a diff result.
fn diff_json(outcome: &verirun_cli::cmd_diff::DiffOutcome) -> serde_json::Value {
    let differences: Vec<serde_json::Value> = outcome
        .differences
        .iter()
        .map(|diff| match &diff.difference {
            TaskDifference::OnlyInFirst => serde_json::json!({
                "task_id": diff.task_id,
                "kind": "only_in_first",
            }),
            TaskDifference::OnlyInSecond => serde_json::json!({
                "task_id": diff.task_id,
                "kind": "only_in_second",
            }),
            TaskDifference::Changed(fields) => serde_json::json!({
                "task_id": diff.task_id,
                "kind": "changed",
                "fields": {
                    "status": fields.status,
                    "exit_code": fields.exit_code,
                    "artifacts": fields.artifacts,
                    "stdout": fields.stdout,
                    "stderr": fields.stderr,
                },
            }),
        })
        .collect();
    serde_json::json!({
        "first": outcome.first.to_hex(),
        "second": outcome.second.to_hex(),
        "equivalent": outcome.differences.is_empty(),
        "differences": differences,
    })
}

fn pin_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let Some(text) = sub.get_one::<String>("digest") else {
        return 3;
    };
    let Some(cas) = sub.get_one::<PathBuf>("cas") else {
        return 3;
    };
    let digest = match parse_digest(text) {
        Ok(digest) => digest,
        Err(error) => return fail(&error, json),
    };
    match verirun_cli::cmd_pin::pin(cas, &digest) {
        Ok(manifest) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "manifest": digest.to_hex(),
                        "merkle_root": manifest.merkle_root.to_hex(),
                        "pinned": true,
                    })
                );
            } else {
                println!("pinned {}", digest.to_hex());
            }
            0
        }
        Err(error) => fail(&error, json),
    }
}

fn unpin_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let Some(text) = sub.get_one::<String>("digest") else {
        return 3;
    };
    let Some(cas) = sub.get_one::<PathBuf>("cas") else {
        return 3;
    };
    let digest = match parse_digest(text) {
        Ok(digest) => digest,
        Err(error) => return fail(&error, json),
    };
    match verirun_cli::cmd_unpin::unpin(cas, &digest) {
        Ok(()) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "manifest": digest.to_hex(),
                        "pinned": false,
                    })
                );
            } else {
                println!("unpinned {}", digest.to_hex());
            }
            0
        }
        Err(error) => fail(&error, json),
    }
}

fn gc_command(sub: &clap::ArgMatches, json: bool) -> i32 {
    let Some(cas) = sub.get_one::<PathBuf>("cas") else {
        return 3;
    };
    let grace_secs = sub.get_one::<u64>("grace_secs").copied().unwrap_or(3600);
    let dry_run = sub.get_flag("dry_run");
    match verirun_cli::cmd_gc::gc(cas, grace_secs, dry_run) {
        Ok(report) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "dry_run": report.dry_run,
                        "objects": report.objects,
                        "object_bytes": report.object_bytes,
                        "tmp_files": report.tmp_files,
                        "tmp_bytes": report.tmp_bytes,
                        "reachable": report.reachable,
                    })
                );
            } else if report.dry_run {
                println!(
                    "dry run: {} objects ({} bytes) and {} temporary files ({} bytes) would be swept; {} objects are reachable from pins",
                    report.objects,
                    report.object_bytes,
                    report.tmp_files,
                    report.tmp_bytes,
                    report.reachable
                );
            } else {
                println!(
                    "swept {} objects ({} bytes) and {} temporary files ({} bytes); {} objects are reachable from pins",
                    report.objects,
                    report.object_bytes,
                    report.tmp_files,
                    report.tmp_bytes,
                    report.reachable
                );
            }
            0
        }
        Err(error) => fail(&error, json),
    }
}
