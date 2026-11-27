#!/bin/sh
# End-to-end CLI walkthrough exercising `verirun run --pin --json`,
# `verirun show --json`, `verirun verify --json`, and
# `verirun gc --dry-run --json` inside an isolated temporary directory.

set -eu

SCRIPT_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "$SCRIPT_DIR/.." && pwd)

resolve_verirun() {
  if [ -n "${VERIRUN_BIN:-}" ]; then
    [ -x "$VERIRUN_BIN" ] || {
      printf 'error: VERIRUN_BIN is not executable: %s\n' "$VERIRUN_BIN" >&2
      exit 1
    }
    printf '%s\n' "$VERIRUN_BIN"
    return 0
  fi
  if [ -x "$REPO_ROOT/target/release/verirun" ] && [ -x "$REPO_ROOT/target/debug/verirun" ]; then
    if [ "$REPO_ROOT/target/release/verirun" -nt "$REPO_ROOT/target/debug/verirun" ]; then
      printf '%s\n' "$REPO_ROOT/target/release/verirun"
    else
      printf '%s\n' "$REPO_ROOT/target/debug/verirun"
    fi
    return 0
  fi
  if [ -x "$REPO_ROOT/target/release/verirun" ]; then
    printf '%s\n' "$REPO_ROOT/target/release/verirun"
    return 0
  fi
  if [ -x "$REPO_ROOT/target/debug/verirun" ]; then
    printf '%s\n' "$REPO_ROOT/target/debug/verirun"
    return 0
  fi
  if command -v verirun >/dev/null 2>&1; then
    command -v verirun
    return 0
  fi
  printf 'error: verirun binary not found; run "cargo build --locked -p verirun-cli --bin verirun" first\n' >&2
  exit 1
}

VERIRUN=$(resolve_verirun)

TMP_ROOT=""
cleanup() {
  if [ -n "$TMP_ROOT" ] && [ -d "$TMP_ROOT" ]; then
    rm -rf "$TMP_ROOT"
  fi
}
trap cleanup EXIT INT TERM HUP
TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/verirun-cli-example.XXXXXX")

CAS_DIR="$TMP_ROOT/cas"
WORK_DIR="$TMP_ROOT/work"
INPUT_FILE="$TMP_ROOT/records.txt"
SPEC_FILE="$TMP_ROOT/pipeline.json"

cat <<'EOF' >"$INPUT_FILE"
alpha
beta
gamma
EOF

cat <<'JSON' >"$SPEC_FILE"
{
  "tasks": [
    {
      "task_id": "normalize",
      "executable": "sh",
      "args": ["-c", "tr a-z A-Z < inputs/records.txt > outputs/upper.txt"],
      "env": {},
      "inputs": ["records.txt"],
      "outputs": ["upper.txt"],
      "depends_on": []
    },
    {
      "task_id": "line_count",
      "executable": "sh",
      "args": ["-c", "awk 'END { print NR }' inputs/upper.txt > outputs/lines.txt"],
      "env": {},
      "inputs": ["upper.txt"],
      "outputs": ["lines.txt"],
      "depends_on": ["normalize"]
    }
  ]
}
JSON

# 1. Cold run with host input binding and `--pin --json`.
RUN_JSON=$("$VERIRUN" run "$SPEC_FILE" \
  --cas "$CAS_DIR" \
  --work "$WORK_DIR" \
  --input "records.txt=$INPUT_FILE" \
  --pin \
  --json)
printf '[1/4] run --pin --json:    %s\n' "$RUN_JSON"

MANIFEST=$(printf '%s\n' "$RUN_JSON" | sed -n 's/.*"manifest":"\([0-9a-f]\{64\}\)".*/\1/p')
[ "${#MANIFEST}" -eq 64 ] || {
  printf 'error: failed to parse 64-hex manifest digest from run JSON\n' >&2
  exit 1
}
case "$RUN_JSON" in
  *'"pinned":true'*) ;;
  *)
    printf 'error: expected pinned=true in run JSON\n' >&2
    exit 1
    ;;
esac

# 2. Inspect manifest and task records via `show --json`.
SHOW_JSON=$("$VERIRUN" show "$MANIFEST" --cas "$CAS_DIR" --json)
printf '[2/4] show --json:         %s\n' "$SHOW_JSON"
case "$SHOW_JSON" in
  *'"task_id":"line_count"'*'"task_id":"normalize"'*) ;;
  *)
    printf 'error: expected sorted task records in show JSON\n' >&2
    exit 1
    ;;
esac

# 3. Re-verify manifest, Merkle root, and blobs via `verify --json`.
VERIFY_JSON=$("$VERIRUN" verify "$MANIFEST" --cas "$CAS_DIR" --json)
printf '[3/4] verify --json:       %s\n' "$VERIFY_JSON"
case "$VERIFY_JSON" in
  *'"verified":true'*) ;;
  *)
    printf 'error: expected verified=true in verify JSON\n' >&2
    exit 1
    ;;
esac

# 4. Run warm without `--pin`, then preview reachability with `gc --dry-run --json`.
"$VERIRUN" run "$SPEC_FILE" \
  --cas "$CAS_DIR" \
  --work "$WORK_DIR" \
  --input "records.txt=$INPUT_FILE" \
  --json >/dev/null

GC_JSON=$("$VERIRUN" gc --cas "$CAS_DIR" --grace-secs 0 --dry-run --json)
printf '[4/4] gc --dry-run --json: %s\n' "$GC_JSON"
case "$GC_JSON" in
  *'"dry_run":true'*) ;;
  *)
    printf 'error: expected dry_run=true in gc JSON\n' >&2
    exit 1
    ;;
esac
