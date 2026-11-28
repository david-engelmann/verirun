//! Shell completion library and CLI surface tests (`INV-CLI-1`,
//! `INV-CLI-2`).
//!
//! Asserts that `verirun completions <SHELL>` requires no
//! `--cas` argument, emits deterministic completion scripts on
//! `stdout` with silent `stderr`, covers every subcommand and
//! documented flag across `bash`, `zsh`, `fish`, and
//! `powershell`, supports `--json` output parity, and exits `3`
//! (never `2`) on unknown or missing shell names.
//!
//! The expected surface is derived from the built binary's own
//! `--help` output, never from a hand-copied list in this file,
//! so a subcommand or flag the parser gains, renames, or drops
//! fails here until the scripts catch up. The two count
//! assertions are the tripwire for a drop, which derivation
//! alone cannot see. When the CLI surface changes on purpose,
//! update the scripts and the counts together.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::process::{Command, Output};

use verirun_cli::{Shell, completions_script};

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verirun"))
        .args(args)
        .env_remove("SIM_SEED")
        .output()
        .expect("spawn verirun")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("exit code")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf8")
}

fn help_text(args: &[&str]) -> String {
    let mut full: Vec<&str> = args.to_vec();
    full.push("--help");
    let out = invoke(&full);
    assert_eq!(code(&out), 0, "help for {args:?} failed: {}", stderr(&out));
    stdout(&out)
}

/// Subcommand names from the top-level help's Commands section,
/// excluding the parser's own `help`, which the scripts complete
/// as a name but which takes no flags of its own to derive.
fn derived_subcommands() -> Vec<String> {
    let text = help_text(&[]);
    let mut names = Vec::new();
    let mut in_commands = false;
    for line in text.lines() {
        if line == "Commands:" {
            in_commands = true;
            continue;
        }
        if in_commands {
            if line.is_empty() {
                break;
            }
            let name = line
                .split_whitespace()
                .next()
                .expect("a Commands line carries a name");
            if name != "help" {
                names.push(name.to_string());
            }
        }
    }
    names
}

/// Every long flag the top-level and per-subcommand help output
/// defines, as one sorted set.
fn derived_flags(subcommands: &[String]) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    let mut helps = vec![help_text(&[])];
    for name in subcommands {
        helps.push(help_text(&[name]));
    }
    for text in &helps {
        for token in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            if token.starts_with("--") && token.len() > 2 {
                flags.insert(token.to_string());
            }
        }
    }
    flags
}

#[test]
fn library_and_cli_completions_cover_all_shells_subcommands_and_flags() {
    let subcommands = derived_subcommands();
    assert_eq!(
        subcommands.len(),
        8,
        "the parser's subcommand surface changed: {subcommands:?}"
    );
    let flags = derived_flags(&subcommands);
    assert_eq!(
        flags.len(),
        11,
        "the parser's flag surface changed: {flags:?}"
    );

    let shells = [
        ("bash", Shell::Bash),
        ("zsh", Shell::Zsh),
        ("fish", Shell::Fish),
        ("powershell", Shell::PowerShell),
    ];

    for (name, variant) in shells {
        assert_eq!(variant.as_str(), name);
        assert_eq!(variant.to_string(), name);
        assert_eq!(name.parse::<Shell>().expect("valid shell"), variant);
        assert_eq!(
            name.to_ascii_uppercase()
                .parse::<Shell>()
                .expect("case-insensitive shell"),
            variant
        );

        let expected = completions_script(variant);
        assert_ne!(expected, "");
        assert!(expected.ends_with('\n'));

        for subcmd in &subcommands {
            assert!(
                expected.contains(subcmd.as_str()),
                "{name} completion script is missing subcommand {subcmd:?}"
            );
        }
        for flag in &flags {
            assert!(
                expected.contains(flag.as_str()),
                "{name} completion script is missing flag {flag:?}"
            );
        }

        // CLI writes raw completion script to stdout with empty stderr and no --cas.
        let out = invoke(&["completions", name]);
        assert_eq!(code(&out), 0, "{name} stderr: {}", stderr(&out));
        assert_eq!(stdout(&out), expected);
        assert_eq!(stderr(&out), "");

        // --json mode emits structured shell + script object.
        let json_out = invoke(&["completions", name, "--json"]);
        assert_eq!(
            code(&json_out),
            0,
            "{name} --json stderr: {}",
            stderr(&json_out)
        );
        assert_eq!(stderr(&json_out), "");
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout(&json_out)).expect("completions --json parses");
        assert_eq!(parsed["shell"], name);
        assert_eq!(parsed["script"], expected);
    }
}

#[test]
fn invalid_and_missing_shell_exit_3_with_json_parity() {
    let missing = invoke(&["completions"]);
    assert_eq!(code(&missing), 3, "missing shell argument must exit 3");

    let missing_json = invoke(&["completions", "--json"]);
    assert_eq!(code(&missing_json), 3);
    let missing_parsed: serde_json::Value =
        serde_json::from_str(&stdout(&missing_json)).expect("missing shell --json parses");
    assert_eq!(missing_parsed["exit_code"], 3);

    for bad in ["tcsh", "nushell", "cmd", ""] {
        assert!(bad.parse::<Shell>().is_err());

        let text_out = invoke(&["completions", bad]);
        assert_eq!(code(&text_out), 3, "bad shell {bad:?} must exit 3");
        assert_eq!(stdout(&text_out), "");
        assert!(
            stderr(&text_out).contains("unsupported shell"),
            "stderr explains unsupported shell for {bad:?}: {}",
            stderr(&text_out)
        );

        let json_out = invoke(&["completions", bad, "--json"]);
        assert_eq!(code(&json_out), 3, "bad shell {bad:?} --json must exit 3");
        assert!(stderr(&json_out).contains("unsupported shell"));
        let parsed: serde_json::Value =
            serde_json::from_str(&stdout(&json_out)).expect("bad shell --json parses");
        assert_eq!(parsed["exit_code"], 3);
        assert!(
            parsed["error"]
                .as_str()
                .is_some_and(|msg| msg.contains("unsupported shell"))
        );
    }
}
