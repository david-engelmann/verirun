//! Shell completion library and CLI surface tests (`INV-CLI-1`,
//! `INV-CLI-2`).
//!
//! Asserts that `verirun completions <SHELL>` requires no
//! `--cas` argument, emits deterministic completion scripts on
//! `stdout` with silent `stderr`, covers every subcommand and
//! documented flag across `bash`, `zsh`, `fish`, and
//! `powershell`, supports `--json` output parity, and exits `3`
//! (never `2`) on unknown or missing shell names.

#![forbid(unsafe_code)]

use std::process::{Command, Output};

use verirun_cli::{Shell, completions_script};

const SUBCOMMANDS: &[&str] = &[
    "run",
    "verify",
    "show",
    "diff",
    "gc",
    "pin",
    "unpin",
    "completions",
];

const FLAGS: &[&str] = &[
    "--cas",
    "--work",
    "--jobs",
    "--input",
    "--pin",
    "--grace-secs",
    "--dry-run",
    "--json",
    "--seed",
    "--help",
    "--version",
];

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

#[test]
fn library_and_cli_completions_cover_all_shells_subcommands_and_flags() {
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

        for subcmd in SUBCOMMANDS {
            assert!(
                expected.contains(subcmd),
                "{name} completion script is missing subcommand {subcmd:?}"
            );
        }
        for flag in FLAGS {
            assert!(
                expected.contains(flag),
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
