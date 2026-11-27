---
name: Bug report
about: Report a determinism, verification, store, runner, or CLI defect
title: ""
labels: bug
assignees: ""
---

## Environment

State the `verirun --version` output, `rustc --version`, Python version (if using the Python SDK), and operating system.

## Reproduction

Because `verirun` is deterministic, defects reproduce from explicit inputs and seeds. Include all applicable details below.

- The `--seed` or `SIM_SEED` value and the `seed:` line printed to standard error
- The smallest run specification JSON or manifest digest that triggers the defect
- The exact `verirun` CLI invocation or Python SDK call
- Any invariant in `docs/INVARIANTS.md` that the observed behaviour violates

## Expected versus actual outcome

State the expected exit code (`0`, `1`, `2`, or `3`), standard output, and standard error alongside the actual outcome observed.
