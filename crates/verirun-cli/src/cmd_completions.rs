//! Deterministic shell completion scripts for `verirun`.
//!
//! Generates static completion definitions for `bash`, `zsh`,
//! `fish`, and `powershell` without adding external build or
//! runtime dependencies. Every script covers the full subcommand
//! and option surface of `verirun`.

use std::fmt;
use std::str::FromStr;

use crate::CliError;

/// Supported interactive shells for completion script generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shell {
    /// GNU Bourne-Again Shell (`bash`).
    Bash,
    /// Z shell (`zsh`).
    Zsh,
    /// Friendly interactive shell (`fish`).
    Fish,
    /// PowerShell (`powershell`).
    PowerShell,
}

impl Shell {
    /// Canonical lowercase name of this shell (`"bash"`, `"zsh"`,
    /// `"fish"`, or `"powershell"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::PowerShell => "powershell",
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Shell {
    type Err = CliError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.eq_ignore_ascii_case("bash") {
            Ok(Self::Bash)
        } else if raw.eq_ignore_ascii_case("zsh") {
            Ok(Self::Zsh)
        } else if raw.eq_ignore_ascii_case("fish") {
            Ok(Self::Fish)
        } else if raw.eq_ignore_ascii_case("powershell") {
            Ok(Self::PowerShell)
        } else {
            Err(CliError::Argument {
                detail: format!(
                    "unsupported shell {raw:?} (expected bash, zsh, fish, or powershell)"
                ),
            })
        }
    }
}

/// Returns the deterministic completion script for `shell`.
///
/// Each script registers tab completions for every `verirun`
/// subcommand (`run`, `verify`, `show`, `diff`, `pin`, `unpin`,
/// `gc`, and `completions`) and their flags (`--cas`, `--work`,
/// `--jobs`, `--input`, `--pin`, `--grace-secs`, `--dry-run`,
/// `--json`, `--seed`, `--help`, and `--version`).
#[must_use]
pub const fn completions_script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => BASH_SCRIPT,
        Shell::Zsh => ZSH_SCRIPT,
        Shell::Fish => FISH_SCRIPT,
        Shell::PowerShell => POWERSHELL_SCRIPT,
    }
}

const BASH_SCRIPT: &str = r#"_verirun() {
    local i cur prev cmd
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd=""

    i=1
    while [ "$i" -lt "$COMP_CWORD" ]; do
        case "${COMP_WORDS[$i]}" in
            --cas|--work|--jobs|--input|--grace-secs|--seed)
                i=$((i + 2))
                continue
                ;;
            run|verify|show|diff|pin|unpin|gc|completions)
                cmd="${COMP_WORDS[$i]}"
                break
                ;;
        esac
        i=$((i + 1))
    done

    case "$prev" in
        --cas|--work)
            COMPREPLY=( $(compgen -d -- "$cur") )
            return 0
            ;;
        --jobs|--input|--grace-secs|--seed)
            return 0
            ;;
    esac

    if [ -z "$cmd" ]; then
        if [[ "$cur" == -* ]]; then
            COMPREPLY=( $(compgen -W "--seed --help -h --version -V" -- "$cur") )
        else
            COMPREPLY=( $(compgen -W "run verify show diff pin unpin gc completions" -- "$cur") )
        fi
        return 0
    fi

    case "$cmd" in
        run)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=( $(compgen -W "--cas --work --jobs --input --pin --json --seed --help -h" -- "$cur") )
            else
                COMPREPLY=( $(compgen -f -- "$cur") )
            fi
            ;;
        verify|show|pin|unpin)
            COMPREPLY=( $(compgen -W "--cas --json --seed --help -h" -- "$cur") )
            ;;
        diff)
            COMPREPLY=( $(compgen -W "--cas --json --seed --help -h" -- "$cur") )
            ;;
        gc)
            COMPREPLY=( $(compgen -W "--cas --grace-secs --dry-run --json --seed --help -h" -- "$cur") )
            ;;
        completions)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=( $(compgen -W "--json --seed --help -h" -- "$cur") )
            else
                COMPREPLY=( $(compgen -W "bash zsh fish powershell" -- "$cur") )
            fi
            ;;
    esac
    return 0
}

complete -F _verirun verirun
"#;

const ZSH_SCRIPT: &str = r#"#compdef verirun

_verirun() {
    local context state state_descr line
    typeset -A opt_args

    local -a commands
    commands=(
        'run:Run a run file and print the manifest digest'
        'verify:Verify a manifest and everything it names'
        'show:Verify a manifest and print its task records'
        'diff:Verify two manifests and compare their records'
        'pin:Verify a manifest and pin it against garbage collection'
        'unpin:Remove a manifest garbage collection pin'
        'gc:Collect unreachable objects older than the grace threshold'
        'completions:Print shell completion script for bash, zsh, fish, or powershell'
    )

    _arguments -C \
        '--seed=[The active seed, printed to stderr on every run]:seed:' \
        '(-h --help)'{-h,--help}'[Print help]' \
        '(-V --version)'{-V,--version}'[Print version]' \
        '1: :->command' \
        '*:: :->args' && return 0

    case "$state" in
        command)
            _describe -t commands 'verirun command' commands
            ;;
        args)
            case "${line[1]}" in
                run)
                    _arguments \
                        '1:run file:_files' \
                        '--cas=[The content-addressed store directory]:cas directory:_files -/' \
                        '--work=[The directory holding task sandboxes]:work directory:_files -/' \
                        '--jobs=[Worker pool size]:jobs:' \
                        '*--input=[Bind a sandbox-relative input path to a host file]:binding:' \
                        '--pin[Pin the produced manifest so garbage collection preserves it]' \
                        '--json[Render the result as one JSON object]' \
                        '--seed=[The active seed, printed to stderr on every run]:seed:' \
                        '(-h --help)'{-h,--help}'[Print help]'
                    ;;
                verify|show|pin|unpin)
                    _arguments \
                        '1:manifest digest:' \
                        '--cas=[The content-addressed store directory]:cas directory:_files -/' \
                        '--json[Render the result as one JSON object]' \
                        '--seed=[The active seed, printed to stderr on every run]:seed:' \
                        '(-h --help)'{-h,--help}'[Print help]'
                    ;;
                diff)
                    _arguments \
                        '1:first manifest digest:' \
                        '2:second manifest digest:' \
                        '--cas=[The content-addressed store directory]:cas directory:_files -/' \
                        '--json[Render the result as one JSON object]' \
                        '--seed=[The active seed, printed to stderr on every run]:seed:' \
                        '(-h --help)'{-h,--help}'[Print help]'
                    ;;
                gc)
                    _arguments \
                        '--cas=[The content-addressed store directory]:cas directory:_files -/' \
                        '--grace-secs=[Grace threshold in seconds]:seconds:' \
                        '--dry-run[Print the plan and delete nothing]' \
                        '--json[Render the result as one JSON object]' \
                        '--seed=[The active seed, printed to stderr on every run]:seed:' \
                        '(-h --help)'{-h,--help}'[Print help]'
                    ;;
                completions)
                    _arguments \
                        '1:shell:(bash zsh fish powershell)' \
                        '--json[Render the result as one JSON object]' \
                        '--seed=[The active seed, printed to stderr on every run]:seed:' \
                        '(-h --help)'{-h,--help}'[Print help]'
                    ;;
            esac
            ;;
    esac
}

if (( $+functions[compdef] )); then
    compdef _verirun verirun
fi
"#;

const FISH_SCRIPT: &str = r"# Fish completions for verirun
# Subcommands: run verify show diff pin unpin gc completions
# Flags: --cas --work --jobs --input --pin --grace-secs --dry-run --json --seed --help --version

function __verirun_subcommand
    set -l words (commandline -opc)
    set -l i 2
    set -l count (count $words)
    while test $i -le $count
        set -l w $words[$i]
        switch $w
            case --cas --work --jobs --input --grace-secs --seed
                set i (math $i + 2)
                continue
            case run verify show diff pin unpin gc completions
                echo $w
                return 0
        end
        set i (math $i + 1)
    end
    return 1
end

function __verirun_needs_subcommand
    not __verirun_subcommand >/dev/null
end

function __verirun_using_subcommand
    set -l cmd (__verirun_subcommand)
    and contains -- $cmd $argv
end

complete -c verirun -l seed -r -d 'The active seed (--seed)'
complete -c verirun -s h -l help -d 'Print help (--help)'
complete -c verirun -n '__verirun_needs_subcommand' -s V -l version -d 'Print version (--version)'

complete -c verirun -n '__verirun_needs_subcommand' -f -a run -d 'Run a run file and print the manifest digest'
complete -c verirun -n '__verirun_needs_subcommand' -f -a verify -d 'Verify a manifest and everything it names'
complete -c verirun -n '__verirun_needs_subcommand' -f -a show -d 'Verify a manifest and print its task records'
complete -c verirun -n '__verirun_needs_subcommand' -f -a diff -d 'Verify two manifests and compare their records'
complete -c verirun -n '__verirun_needs_subcommand' -f -a pin -d 'Verify a manifest and pin it against garbage collection'
complete -c verirun -n '__verirun_needs_subcommand' -f -a unpin -d 'Remove a manifest garbage collection pin'
complete -c verirun -n '__verirun_needs_subcommand' -f -a gc -d 'Collect unreachable objects older than the grace threshold'
complete -c verirun -n '__verirun_needs_subcommand' -f -a completions -d 'Print shell completion script'

complete -c verirun -n '__verirun_using_subcommand run verify show diff pin unpin gc' -l cas -r -a '(__fish_complete_directories)' -d 'Content-addressed store directory (--cas)'
complete -c verirun -n '__verirun_using_subcommand run verify show diff pin unpin gc completions' -l json -d 'Render the result as one JSON object (--json)'

complete -c verirun -n '__verirun_using_subcommand run' -F -l work -r -a '(__fish_complete_directories)' -d 'Task sandbox directory (--work)'
complete -c verirun -n '__verirun_using_subcommand run' -l jobs -r -d 'Worker pool size (--jobs)'
complete -c verirun -n '__verirun_using_subcommand run' -l input -r -d 'Bind sandbox input path to host file (--input)'
complete -c verirun -n '__verirun_using_subcommand run' -l pin -d 'Pin produced manifest (--pin)'

complete -c verirun -n '__verirun_using_subcommand verify show diff pin unpin' -f

complete -c verirun -n '__verirun_using_subcommand gc' -f -l grace-secs -r -d 'Grace threshold in seconds (--grace-secs)'
complete -c verirun -n '__verirun_using_subcommand gc' -f -l dry-run -d 'Print the plan and delete nothing (--dry-run)'

complete -c verirun -n '__verirun_using_subcommand completions' -f -a 'bash zsh fish powershell'
";

const POWERSHELL_SCRIPT: &str = r#"Register-ArgumentCompleter -Native -CommandName verirun -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $elements = $commandAst.CommandElements | ForEach-Object { $_.Extent.Text }
    $subcommands = @('run', 'verify', 'show', 'diff', 'pin', 'unpin', 'gc', 'completions')
    $activeSubcommand = $null

    for ($i = 1; $i -lt $elements.Count; $i++) {
        $token = $elements[$i]
        if ($token -eq $wordToComplete -and $i -eq ($elements.Count - 1)) {
            break
        }
        if ($token -in @('--cas', '--work', '--jobs', '--input', '--grace-secs', '--seed')) {
            $i++
            continue
        }
        if ($token -in $subcommands) {
            $activeSubcommand = $token
            break
        }
    }

    $candidates = if ($null -eq $activeSubcommand) {
        if ($wordToComplete.StartsWith('-')) {
            @('--seed', '--help', '-h', '--version', '-V')
        } else {
            $subcommands
        }
    } else {
        switch ($activeSubcommand) {
            'run' { @('--cas', '--work', '--jobs', '--input', '--pin', '--json', '--seed', '--help', '-h') }
            'verify' { @('--cas', '--json', '--seed', '--help', '-h') }
            'show' { @('--cas', '--json', '--seed', '--help', '-h') }
            'diff' { @('--cas', '--json', '--seed', '--help', '-h') }
            'pin' { @('--cas', '--json', '--seed', '--help', '-h') }
            'unpin' { @('--cas', '--json', '--seed', '--help', '-h') }
            'gc' { @('--cas', '--grace-secs', '--dry-run', '--json', '--seed', '--help', '-h') }
            'completions' {
                if ($wordToComplete.StartsWith('-')) {
                    @('--json', '--seed', '--help', '-h')
                } else {
                    @('bash', 'zsh', 'fish', 'powershell')
                }
            }
            default { @() }
        }
    }

    $candidates | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
        [System.Management.Automation.CompletionResult]::new($_, $_, 'ParameterValue', $_)
    }
}
"#;
