# Quickstart

## Install `verirun`

Install the prebuilt binary on Linux or macOS (`x86_64` and `aarch64`) with SHA-256 verification against `SHA256SUMS`:

```sh
curl -fsSL https://raw.githubusercontent.com/david-engelmann/verirun/main/install.sh | sh
```

Or install the bundled binary + typed Python SDK from PyPI, run directly with `uvx`, or compile from crates.io:

```sh
pip install verirun
# or run without installing into an environment
uvx verirun --version
# or install from crates.io
cargo install verirun-cli
```

Enable tab completions for `bash`, `zsh`, `fish`, or `powershell`:

```sh
# Bash
eval "$(verirun completions bash)"

# Zsh
source <(verirun completions zsh)

# Fish
verirun completions fish | source

# PowerShell
verirun completions powershell | Out-String | Invoke-Expression
```

## Run a deterministic pipeline from Python

The `verirun` Python package (compatible with Python 3.10 and newer, using only the Python standard library) validates task graphs upfront and invokes the `verirun` binary in `--json` mode:

```python
import verirun

pipeline = (
    verirun.PipelineBuilder()
    .add_task(
        task_id="greet",
        executable="sh",
        args=["-c", "printf 'hello from verirun\\n' > outputs/greeting.txt"],
        outputs=["greeting.txt"],
    )
    .add_task(
        task_id="shout",
        executable="sh",
        args=["-c", "tr a-z A-Z < inputs/greeting.txt > outputs/loud.txt"],
        inputs=["greeting.txt"],
        outputs=["loud.txt"],
        depends_on=["greet"],
    )
)

result = pipeline.run(cas="./cas", work="./work", pin_manifest=True, check=True)
verified = verirun.verify(result.manifest, cas="./cas")
print("manifest:", verified.manifest)
print("merkle_root:", verified.merkle_root)
```

## Run the same graph from the CLI

Save the task specification as `pipeline.json`:

```json
{
  "tasks": [
    {
      "task_id": "greet",
      "executable": "sh",
      "args": ["-c", "printf 'hello from verirun\n' > outputs/greeting.txt"],
      "env": {},
      "inputs": [],
      "outputs": ["greeting.txt"],
      "depends_on": []
    },
    {
      "task_id": "shout",
      "executable": "sh",
      "args": ["-c", "tr a-z A-Z < inputs/greeting.txt > outputs/loud.txt"],
      "env": {},
      "inputs": ["greeting.txt"],
      "outputs": ["loud.txt"],
      "depends_on": ["greet"]
    }
  ]
}
```

Execute, inspect, and verify:

```sh
MANIFEST="$(verirun run --cas ./cas --work ./work --pin pipeline.json)"
verirun show --cas ./cas "$MANIFEST"
verirun verify --cas ./cas --json "$MANIFEST"
```

## Runnable repository examples

The repository ships self-contained runnable scripts in [`examples/`](https://github.com/david-engelmann/verirun/tree/main/examples) that are executed continuously in CI:

- [`examples/reproducible_eval_pipeline.py`](https://github.com/david-engelmann/verirun/blob/main/examples/reproducible_eval_pipeline.py): 3-task evaluation DAG (`prepare_dataset` -> `score_candidates` -> `summarize_report`) proving worker-count determinism, warm `by-exec` caching, manifest diffing, and 1-byte tamper conviction (`exit_code == 2`).
- [`examples/run_and_verify.sh`](https://github.com/david-engelmann/verirun/blob/main/examples/run_and_verify.sh): POSIX shell CLI walkthrough exercising `run --pin --json`, `show --json`, `verify --json`, and `gc --dry-run --json`.

Next, read the [Step-by-step tutorial](docs/TUTORIAL.md) or the [CLI and Python SDK reference](./cli-and-python.md).
