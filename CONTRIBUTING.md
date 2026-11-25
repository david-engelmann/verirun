# Contributing to verirun

verirun is built in milestones, and every milestone lands in two stages. Stage A lands the public interfaces with doc comments and the tests that pin the new behaviour, failing where the behaviour does not exist yet. Stage B implements until the whole suite is green and brings the documents back into sync. A milestone is finished only when its report lists the invariants it verified, the files it changed, and the exact commands that were run with their pass counts.

## Before you push

Run the same gates the hosted workflow runs, in the same order, from the workspace root.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
PROPTEST_CASES=1000 cargo test --workspace --all-features --locked --no-fail-fast
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo bench --workspace --locked -- --test
```

Property test volume comes from the environment. Pull requests run one thousand cases and the main branch runs ten thousand, and no test may hardcode a smaller count of its own.

The core crate is standard library only, and that is a hard rule rather than a preference. External crates belong in the OS boundary crate, the I/O adapters, the command line crate, benchmarks, and dev-dependencies, and each one needs a reason a reader can find. Deterministic code never iterates a hash map, never reads the wall clock, and never draws operating system randomness. `verirun-platform` is the sole crate isolating operating system FFI unsafe blocks behind a safe API with `unsafe_op_in_unsafe_fn` denied, and every other crate and every test target forbids unsafe code.

## Tests that can fail

A new test must be able to fail for a real reason. Tests that only re-derive their expectation from the code under test prove nothing and do not land. Fault tests name the corruption they inject and the exact failure they expect, down to the exit code and the digests in the message. Stress tests run under a deadline inside the test, so a deadlock fails the suite instead of hanging it.

## Writing

Public prose follows a short style bar. No em dashes, no colons, and no semicolons in prose. Code blocks, commands, and error text quoted from the tools are exempt because they are literals rather than prose. Write plainly, say what the software does and how it is used, and keep the honest current state in the README rather than in release notes nobody reads.

## Commits

One branch per milestone stage, one worktree per branch, and every commit carries the sign-off trailer.

```text
Signed-off-by: David Engelmann <81196676+david-engelmann@users.noreply.github.com>
```

Push the branch early and let the hosted workflow run while you keep working. A milestone lands on the main branch only after the branch head is green in the hosted workflow, and the landing is verified on the main branch itself afterwards.

## Versioning

The four crates version in lockstep, so one version number names the whole workspace state and the inter-crate requirements always match the workspace version. Releases are tagged with that version and carry the matching changelog section as their notes.

Some surfaces are forever, and no version bump may move them. The domain tags and digest framing, the canonical encoding rules, and the manifest semantics defined in the architecture document are the product's memory. A manifest written by any published version verifies under every later version, because a store that outlives the binary that wrote it is the point of the design. If a format change ever became unavoidable, it would arrive as a new domain era alongside the old one, never as a silent change under the same tags.

Before 1.0, a minor bump may break library APIs and command line details, and a patch bump carries fixes that change no behaviour a test pins. The forever surfaces are already frozen in this period, from the first published version onward. From 1.0 onward, plain semantic versioning applies to everything outside the forever set. Breaking changes take a major bump, new capability takes a minor bump, and fixes take a patch bump.

## Checkpoints

Before a milestone is called done, the finished state gets three reviews. An independent re-audit of the finished tree against every claim in the report, a line-by-line review of the diff on two axes at once (whether it implements the spec, and whether it meets the standards above), and the owner's own verification pass with the key commands re-run firsthand. Findings get fixed before the landing, not after.
