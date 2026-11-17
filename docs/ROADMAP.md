# verirun Roadmap

Status: post-Milestone 4. The four-milestone build is complete and fully gated. This roadmap is the ordered forward program for the repo. It was written from a firsthand audit of the finished build on 2026-11-16, and every lane below names the audit finding it answers.

## How lanes work

Lanes run in order, one at a time. A lane lands through the normal gate: a branch, a pull request, hosted CI green on the exact head, then one landing commit on main with the reviewed tree. Behaviour changes add or amend rows in docs/INVARIANTS.md in the same pull request. The standard-library-only rule for verirun-core binds every lane. Performance claims come from the benchmark suite run before and after, never from estimates. Direction calls that only the project owner can settle are recorded at the end of this document. They gate the acts they name and nothing else, so preparation work proceeds without them.

## R1. Publication readiness

The audit found the manifests close to publishable. Versions, descriptions, keywords, categories, licence, and repository are all present. Four gaps remain. The inter-crate dependencies name a path and no version, so packaging warns that the published dependency will resolve from the registry instead of the workspace. No crate package carries a readme, so the crates.io pages would render bare. The declared rust-version of 1.85 has never been verified by a build. Lane R1 verified it and found it false in both directions: the store uses let-chains, stable since Rust 1.88, and the locked proptest requires 1.88, so the declared floor is now 1.88, proven by a full local suite run at that version (80 passed, 0 failed). There is no changelog. The names verirun, verirun-core, verirun-store, and verirun-cli were all free on crates.io when checked on 2026-11-16.

Scope: put an explicit version on every inter-crate dependency alongside its path. Wire a readme into each crate package and add documentation and homepage fields. Start CHANGELOG.md with a 0.1.0 entry that summarizes what Milestones 1 to 4 shipped. Add a CI job that builds and tests at the declared rust-version. Rehearse publication with cargo package for all three crates, a review of each package's contents, and cargo publish in dry-run mode up to the authentication step, in dependency order.

Gate: cargo package is clean for all three crates with contents reviewed and the dry-run rehearsal passes. For store and cli the registry side of that verification completes in dependency order at publication, because the registry cannot resolve an unpublished sibling. CI is green including the rust-version job. The publication act itself is direction call D1.

## R2. Examples and documentation depth

The audit found no examples anywhere in the repo and no lint that holds public documentation to completeness. A new user can read the architecture document but cannot run a five-line program against any crate.

Scope: an examples directory in each library crate, covering canonical encoding and digest framing in core and put, read, and collection in store. The command line program's guided run is carried by the tutorial instead of an example, because an example binary cannot invoke the shipped binary without path tricks an example should not teach. A tutorial under docs/ that walks one run file from writing it to verifying and diffing its manifests, with output captured from the real program. The missing_docs lint denied workspace-wide, with every gap it exposes documented. Intra-doc links exercised by the existing doc gate.

Gate: CI compiles and runs the examples, the doc gate passes with missing_docs denied, and the full suite stays green. Lane R2 found the public API already fully documented, so the lint landed with zero fallout and a positive control proving it fires.

## R3. Fault and stress growth

The M4 suites cover tamper, crash recovery at put boundaries, and pipe stress. The audit found three shapes still unproven. Collection interrupted mid-sweep has no fault, although the design claims a rerun completes safely. A full disk during a put has no fault, although the bench work hit exactly that failure in practice. The INV-LAT-2 edge case of a child that never closes its streams is covered only in smaller shapes.

Scope: a collection interruption fault that stops a sweep between deletions and proves the store stays consistent and a rerun finishes the plan. A disk-full fault on a bounded store proving the put fails as a named store error with no partial object visible. The never-closes pipe shape at the full 16 MiB the invariant names. A scheduled weekly workflow that runs the stress tier at ten times the pull request volume, with the heaviest tests admitted to that tier by name.

Gate: each fault lands with its INVARIANTS matrix row in the same pull request, and the weekly workflow is green on its first scheduled run. Lane R3 landed in two slices on 2026-11-16. INV-GC-5 and the full-size INV-LAT-2 edge landed first (main 54234c89), then INV-CAS-9 with the stress workflow (main 25ffe42c). The workflow's first dispatched run (43921605698) failed in its 100,000-case suite step. The cause was a generator defect in prop_tuple_framing_injective, which drew two field lists independently and filtered equal pairs until the reject cap aborted the run at the higher volume, exactly the class of defect the volume tier exists to catch. Hosted bisection cleared the other five property suites at 100,000 cases, the generator now derives the second list from the first with one guaranteed change (main 78ad52a7), and the re-dispatched run 43969419775 completed green with the full suite and the tenfold stress suite both passing. The temporary diag workflow added for the bisection is removed in the same change that records this closure.

## R4. Performance

The recorded figures are honest and, by the gate audit's own reruns, conservative. The audit found three places where the implementation spends more than the design requires. put_bytes writes and syncs a full temp file before the presence check, although the digest of a byte slice is known before any write, so a repeat put pays the whole durability cost to learn the object was already there. Manifest verification reads blobs one at a time. The canonical encoder's allocation profile has never been measured.

Scope: a presence pre-check in put_bytes that skips the temp write when the digest is already stored, with the safety argument stated in the code (the digest is computed from the caller's bytes before the check, and presence under a content address asserts nothing the read path does not re-verify). Parallel blob reads in verification with error reporting kept in first-reference order. An allocation pass over the encoder measured with the counting sink the core already ships. Every change records before and after benchmark numbers in its pull request, and the README figures are refreshed in the same pull request when a number moves beyond run-to-run noise.

Gate: numbers recorded for each change, the full suite green, and no invariant amended. A change that cannot show a measured improvement does not land. Lane R4 landed in three slices on 2026-11-16. The put_bytes presence pre-check took repeat puts of one 64 KiB payload from 14.8 and 15.3 puts/s before to 1233.1 and 1631.8 after (main 734eaf81). The mixed bench stayed inside its noise band, so no mixed figure is claimed and the README numbers stand. Parallel verification (main acdb7795) at a 128-task manifest measured 3.61 and 4.53 verifications/s sequentially and 8.47 and 6.00 in parallel, while at 32 tasks the two were indistinguishable, so the slice is claimed only at the size where it pays. The encoder allocation pass (main 9e160d5b) measured 13.8, 21.7 and 20.4 MiB/s before and 28.6, 25.9 and 26.0 after on the shipped bench. No invariant was amended.

## R5. CLI surface completion

The audit found one surface gap. Verification answers whether a manifest is sound and diff answers whether two runs match, but nothing lets a user look at what a manifest says without writing code against the library.

Scope: a `verirun show` command that prints a manifest's tasks, statuses, exit codes, and digests in text, with the same content under --json. Show verifies the manifest under section 9.3 of the architecture document before displaying anything, so it never presents an unverified manifest as fact, and its failures take the existing exit code for verification. A CLI invariant row records the verify-first rule, with exit code table tests for the new command.

Gate: the command ships with its matrix row and tests, and the full suite stays green. Lane R5 landed on 2026-11-16 (main 639ec25c). The show command verifies under section 9.3 before rendering, its matrix row is INV-CLI-5, and its oracles include the tampered-store case exiting 2 with empty stdout.

## R6. Portability

The core is pure computation and the store's platform assumptions are already fenced behind cfg boundaries, but nothing proves either claim on a second platform. The runner is POSIX by design, through process groups and shell execution, and that boundary is currently implicit.

Scope: Windows compile coverage for verirun-core and for the parts of verirun-store outside the runner, a windows-latest CI job for exactly those crates, and the runner's POSIX boundary stated plainly in the architecture document. Runtime support on Windows is direction call D3 and is not assumed by this lane.

Gate: the Windows CI job is green for the crates it claims, and no POSIX behaviour changes. Lane R6 landed on 2026-11-16 (main a61b5dcd). The windows job was green on its first run, checking core and store and running the core tests on windows-latest, and the Linux suite was unchanged and green beside it.

## R7. Release engineering

Nothing watches the dependency set for advisories, and there is no release discipline written down for the day publication happens.

Scope: cargo audit running in CI with its failures treated as gate failures. A written semantic versioning policy in CONTRIBUTING.md that spells out what the project already treats as forever (the domain-separated digest formats and the canonical encoding) and what may still move before 1.0. A GitHub release for each published version carrying its changelog section as the notes, with tags matching crate versions.

Gate: the audit job is green, the policy is written, and the first release is cut when D1 is decided. Lane R7 landed on 2026-11-16 (main c825cee1). The audit job runs cargo audit in CI and was green on its first run, the versioning policy stands in CONTRIBUTING.md, and the dispatch-only release workflow is ready. D1 was decided on 2026-11-16, and the first release is cut with the 0.1.0 publication.

## Direction calls recorded

D1. Publication. Decided by David on 2026-11-16 and executed that night. Verirun-core, verirun-store and verirun-cli are live on crates.io at 0.1.0, published in dependency order through the owner's connected credential and verified firsthand from the crate pages, the sparse index, the docs.rs builds and a clean-runner consumer build that added all three crates to a fresh project and installed the binary. The GitHub release v0.1.0 carries the 0.1.0 changelog section as its notes. The name verirun itself is the binary the cli crate installs, so three crates carry the release.

D2. The 1.0 line. When the three crates and the CLI surface freeze under semantic versioning is a product decision. R7 writes the policy that describes the freeze; choosing its date is the owner's.

D3. Windows runtime support. The runner's process model is POSIX at its foundation. Whether running tasks on Windows is ever a goal is a product decision. R6 proves compilation only.
