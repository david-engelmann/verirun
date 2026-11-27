# Verirun successor programme: register

The register of the successor programme chartered in
docs/SUCCESSOR_ROADMAP.md, approved in full by Main Muse on 2026-11-26.
Every finding is dispositioned LAND, DEFER with reason, or REJECT with
reason. A slice closes only on an entry here carrying its gate evidence,
so no slice closes by paperwork alone. Hosted evidence is the gate of
record throughout.

## Standing record carried into this programme

These items stand settled from the audit and perfection programmes and
are carried here as record, not as work. None of them re-opens without a
ruling from Main. PF-1 stands accepted as a platform fact (directory
entry durability is weaker on Windows, and every read rehashes, so a
lost entry reads as missing and never as corrupt). PF-2, PF-3, PF-7, and
PF-8 stand rejected with their reasons in docs/PERFECTION_REGISTER.md.
F10 and F22 stand rejected with their reasons in docs/AUDIT_REGISTER.md.
The closure audit's nit N-3 stands accepted as recorded (the wall-clock
oracle tests, whose mechanisms are time based and whose waits are
documented at their sites). DECISIONS.md stands exactly as the 0.2.0
launch left it, and the closure audit's flags on it stand recorded in
the perfection register as their deliverable.

## Slice ledger

- S2.0 Published-state ground truth. CLOSED 2026-11-26, no finding,
  on the evidence recorded in its section below. Seated first, on
  Main's sequencing
  note, since the later slices of S1 and S2 reason from what stands
  published.
- S1.1 The contract read. CLOSED 2026-11-26, no finding, on the
  correspondence recorded in its section below.
- S1.2 Binary discovery. CLOSED 2026-11-26, one finding raised and
  deferred to S1.4 with its remedy, on the probe evidence recorded in
  its section below.
- S1.3 Error mapping. CLOSED 2026-11-26, no finding, on the mapping
  recorded in its section below.
- S1.4 Fault-path depth and the Python floor. CLOSED 2026-11-26, the
  fault suite and the floor leg landed and S1.2's deferred remedy with
  them, on the evidence recorded in its section below.
- S2.1 The publish chain read. CLOSED 2026-11-27, no finding, the
  account-side confirmations named in its section below for Main or
  David.
- S2.2 Shipped-binary verification, per channel. CLOSED 2026-11-27,
  no finding, on the hosted proof recorded in its section below.
- S2.3 Version coherence, extended. CLOSED 2026-11-27, the parity
  checks extended to every version home and to pull request time,
  on the evidence recorded in its section below.
- S3.1 The macOS probe. Queued.
- S3.2 The Windows parity re-proof. Queued.
- S4.1 The security file and the remaining surfaces. Queued.

## S2.0: Published-state ground truth

Seated 2026-11-26 on the banking of the roadmap and this register. The
slice establishes, per channel, what stands published under the name
verirun. On crates.io, on PyPI, and as GitHub Release assets it records
published or not, the version, the asset inventory, and an install-and-run
from the index in a clean hosted environment. A broken or partial 0.2.0
on any channel is a finding of this slice.

Closed 2026-11-26. Hosted evidence: diagnostic run 45900132344 on
ubuntu-latest for the index probes and the install-and-run, the
crates.io API and sparse index read the same day, and the GitHub
releases API read the same day. On crates.io, each of verirun-core,
verirun-store, and verirun-cli carries 0.1.0 and 0.2.0, with 0.2.0
newest. On PyPI, the verirun project carries 0.2.0 only, the SDK having
not existed at 0.1.0, as the source distribution and five py3-none
wheels (win_amd64, manylinux_2_28 x86_64 and aarch64, macosx_11_0_arm64,
and macosx_10_12_x86_64). A clean hosted virtual environment installed
verirun 0.2.0 from the index, the SDK reported its own version as
0.2.0, and the bundled binary reported 0.2.0 through the SDK. On GitHub
Releases, v0.2.0 carries SHA256SUMS and the five platform archives, and
the linux x86_64 archive verified against the sums file, contains the
binary with the README and both licences, and runs. The v0.1.0 release
carries no assets today, recorded here as observed and nothing more.
Disposition: no broken or partial 0.2.0 stands on any channel, the
three channels agree on the version, and the launch's publish machinery
completed what PR 223 repaired it to complete. No finding is raised.
The ground truth the rest of S1 and S2 reason from is this entry.

## S2.1: The publish chain read

Closed 2026-11-27. The four release-side workflows read end to end
at main 4a9938e8, with their run conclusions read the same day.
ci.yml stands as every slice's gate evidence and is not this
slice's subject.

release.yml cuts a GitHub release from a version tag or a
dispatch. Its prepare job verifies the four crate manifests and
the inter-crate dependency versions against the release version
and extracts the matching changelog section, failing without an
exact version heading. Five platform legs build the archives and
check the binary's version output where the leg is native, and
the publish job asserts the five expected archives exist, writes
SHA256SUMS over them, and undrafts the release. Two runs stand,
both successful, the 0.1.0 dispatch of 2026-11-17 and the 0.2.0
tag push of 2026-11-26.

publish.yml carries crates.io. Its package job runs the broadest
parity check in the tree, taking the pyproject version as the
expected value across the four crates, their inter-crate
dependencies, and the tag itself. Its publish job runs under the
crates-io environment, prefers the registry token secret when
present with OIDC trusted publishing as the fallback, and
publishes in topological order with already-published crates
skipped and sparse-index visibility waits between crates. One run
stands, the 0.2.0 tag push, successful. Its step record settles
one question the tree cannot: the OIDC step was skipped and the
secret check passed, so the 0.2.0 crates.io publish travelled the
token route, not trusted publishing.

pypi.yml builds five wheels and the source distribution with
maturin, smoke-tests the installed wheel in a clean virtual
environment on Linux, macOS, and Windows, and publishes under the
pypi environment with OIDC and a token fallback, skipping
existing files. One run stands, the 0.2.0 tag push, successful.
Which route its publish step took is not separable from the run
record, the step running identically either way.

stress.yml runs the weekly tier, Mondays at 05:17 UTC and on
dispatch, the full suite at 100,000 property cases and the pipe
stress suite ten times under a 300-minute timeout. Three runs
stand: a dispatch failure on 2026-11-16 in the roadmap era, its
successful re-dispatch the same evening, and the scheduled run of
2026-11-23, successful.

The account-side confirmations the tree and the run records
cannot give are named here for Main or David, unresolved by this
slice: whether the pypi and crates-io environments carry
protection rules, whether trusted publishing is registered for
the project on PyPI's side, whether trusted publishing stands
configured per crate on crates.io's side today and whether it is
to be configured there now that the token route
is proven to be the one carrying publishes, and whether release
immutability or tag protection stands on the repository.
Disposition: the chain did what it claims at 0.2.0 on every
channel, no finding is raised, and the two gaps this read
confirms pass to their owning slices, the wheel proof to S2.2 and
the parity gaps to S2.3 (the SDK's own version literal checked by
no workflow, and parity checked only at release time, never at
pull request time).

## S2.2: Shipped-binary verification, per channel

Closed 2026-11-27. The builds under proof are pypi.yml's own: the
workflow was dispatched on main f0d6fbf0 with publishing disabled
(run 45958953858, all five wheel legs and the source distribution
built, the three smoke tests green, the publish job skipped). A
diagnostic workflow then installed the artifacts in clean
virtual environments on runners of their own architecture and
drove the portable spec from the platform identity suite through
the installed SDK, five of the six in that form, with the sixth
handled as this entry records below. The criterion is the one
the scope decided: the
binary's version string and the portable spec's manifest digest
against the pinned platform-free commitment, per channel against
its own build inputs, with no byte-for-byte claim between
channels.

Five channels pass in that form: the linux x86_64, linux aarch64,
macOS aarch64, and windows x86_64 wheels, and the source
distribution built from source on Linux. On each, the SDK
reported 0.2.0, the bundled binary reported 0.2.0, and the run's
manifest digest equalled the commitment exactly (diagnostic run
45959386921).

The sixth channel, the macOS x86_64 wheel, cannot be installed by
pip on the current hosted fleet: the macOS runners are arm64
only, and the x86_64 interpreter the setup action offers there
reports itself arm64 universal2, so pip finds no matching
distribution (proven from the runner, not inferred). That channel
was proven at the binary instead: the wheel unpacked, its bundled
binary is a Mach-O x86_64 executable, and under Rosetta it
reports 0.2.0 and produces the same pinned manifest digest
through the command line (run 45960492377). Disposition: every
channel pypi.yml ships carries a binary that identifies itself
as 0.2.0 and reproduces the platform-free manifest, no finding is
raised, and the one channel whose install route the hosted fleet
cannot exercise stands proven at the binary with the reason on
the record.

Completion, recorded at the workstream's closure on 2026-11-27:
the closure reviews found this entry's proof covered one of the
two build channels its charge names, the release archives built
by release.yml standing unexamined under the criterion, and the
omission is corrected here rather than defended. The five
published v0.2.0 archives were downloaded on runners of their
own architecture, each verified against the release's
SHA256SUMS, and each archive's binary was put through the same
criterion (run 45985768062): all five report version 0.2.0 and
produce the pinned manifest digest exactly, the macOS x86_64
archive at the binary under Rosetta as with its wheel. With
that, both channels the charge names stand proven, and S2.0's
ground truth gains the functional half its inventory lacked,
every published archive now checksummed and run.

## S2.3: Version coherence, extended

Closed 2026-11-27. S2.1's read confirmed two gaps in the parity
net: the SDK's own version literal was checked by no workflow,
and parity ran only at release time, never at pull request time,
so a skew introduced in a pull request passed the full gate and
surfaced only when a release was cut. This slice closes both.

The ci.yml hygiene job gains a version parity step that runs on
every pull request, merge group, and push to main: the pyproject
version as the reference, checked against the SDK's version
literal, the four crate package versions, and every inter-crate
dependency version. release.yml's prepare step now checks the
pyproject version and the SDK's literal against the release
version beside the crate checks it already ran, and publish.yml's
parity step checks the SDK's literal against its pyproject
reference. Every version home the tree carries now stands under
a check at pull request time and at release time both.

The new checks were proven able to fail before they were trusted:
the pull request script and the release script were each run
locally against the true tree (pass, at 0.2.0) and against skewed copies
(an SDK literal at 0.2.1, a crate version at 0.2.1, a pyproject
version at 0.2.1), each failing with the exact mismatch named.
The publish step's added lines share the pull request script's
proven shape and first run at the next crates.io publish. Two
boundaries stand on the record: the generated version entries in
Cargo.lock are read by no parity script and stand covered
transitively, because every cargo command in the gate runs
locked, and the three scripts walk each manifest's top level
dependencies table, which is where every inter-crate pin lives
today.
Disposition: the gaps are closed, no finding is raised, and
workstream S2 stands complete with S2.0's ground truth beneath
it.

## S2 closure

Workstream S2 closed 2026-11-27 under the checkpoint rule's
three reviews. The red team found three of the workstream's
four closure claims surviving as written and one overstatement,
S2.2's single-channel proof, with S2.0's no-broken verdict
resting at its writing on single-platform functional evidence.
Spec fidelity found S2.0, S2.1, and S2.3 meeting their charges
and S2.2 partial on the same gap. Standards found no blockers,
its nits being the changelog's one internal term, one inexact
error message in release.yml, and wording precision in two
entries. Every finding was repaired in the closure pass that
carries this section: the archives channel proven and recorded
in S2.2's entry, the account-side list in S2.1's entry restored
to its charge's full form, S2.3's boundaries stated in its
entry, the changelog bullet reworded, and the release message
split to match its siblings. My own verification re-read each
repair against its review and re-checked the closure claims
against the tree.

## S1.1: The contract read

Closed 2026-11-26, no finding. The read covered both sides of every
--json payload at main 94c66da4. On the CLI side the emitters are
run_json, the verify form, show_json, diff_json, and the pin, unpin,
and gc forms in crates/verirun-cli/src/main.rs, with error_json in
crates/verirun-cli/src/lib.rs. On the SDK side the parsers are the nine
public functions in python/verirun/__init__.py. Field by field, every
success payload corresponds exactly, in names, nesting, and types. The
run form's manifest, exit code, pinned flag, task rows, and failure
rows, the verify form's manifest, verified flag, and Merkle root, the
show form's per-task records with their artifact rows, the diff form's
equivalence flag and per-task difference rows with their five field
flags, the pin and unpin forms, and the gc form's six counters are each
emitted by the CLI and read by the SDK under the same names. The SDK
carries two defensive defaults, over the run form's exit code and
pinned flag, for fields the CLI always emits. The error object's three
fields the SDK reads, its error text, exit code, and kind, match
error_json, with the kind present for the verification family and the
kind-specific fields carried to consumers untyped on the exception's
payload. One ambiguity was checked and closed on the code. The SDK
admits exit code 1 as a result for run and diff, and the CLI's exit
code map yields only 2 or 3 for every error variant, exit 1 arising
only from outcomes, so an error object can never arrive under a code
the SDK treats as a result. Disposition: the contract holds at this
tree with no drift in either direction, and the correspondence this
entry records is the baseline the error mapping read in S1.3 and any
future payload change are judged against.

## S1.2: Binary discovery

Closed 2026-11-26, one finding raised and deferred to S1.4. Hosted
evidence: diagnostic run 45913524671 on ubuntu-latest. Staging note:
the scope named the published v0.1.0 release binaries, and the v0.1.0
GitHub release carries no assets (S2.0), so the 0.1.0 binary was
staged by cargo install from the published crates.io crate, the same
published lineage by another route. The 0.2.0 binary was staged the
same way, and the SDK was the PyPI 0.2.0 install. The probe found the
discovery order working as the code reads. The VERIRUN_BIN override
won over every later candidate, and with it unset the install's own
scripts directory candidate won ahead of PATH, PATH standing last by
construction in the code. The SDK reported each binary's version
without complaint, there being no version check anywhere in the
discovery path. Cross-version behaviour split cleanly. The 0.1.0
binary verified a manifest the 0.2.0 binary had produced and pinned,
the forever surface holding in the backward direction through the new
SDK. The two feature gaps failed loudly rather than silently. A run
with bound inputs against the 0.1.0 binary failed on its rejection of
the --input argument, and pin against it failed on the unrecognized
subcommand, each surfacing the older CLI's own text through the SDK's
exception at exit code 3. Finding SF-S1-1: nothing in that path names
the actual cause, a version mismatch between the SDK and the binary
it found, so a user holding a stale binary meets a raw argument error
instead of a version statement. A hard version refusal is not the
remedy, since the probe proves an older binary is correct by design
for the manifest-reading functions. The remedy charged to S1.4, whose
wrong-version fault fixture it joins, is diagnostic: on a payload-less
failure the SDK names the resolved binary's version alongside the
error, so the mismatch states itself without refusing cross-version
work. Disposition: the discovery order and the cross-version contract
stand as designed, and the finding rides to S1.4.

## S1.3: Error mapping

Closed 2026-11-26, no finding. The read covered the CLI's error
surface against the SDK's exception at main 92f55ba4, on top of S1.1's
payload census. The CLI's side is error_json and the exit code map in
crates/verirun-cli, and the SDK's side is the VerirunError
construction in _invoke_json, in version, and in RunResult's
raise_for_failures. The mapping is faithful and lossless. When the
CLI emits its error object, the SDK raises with the object's own
message text, its exit code (the process code standing as fallback,
and the two cannot disagree since error_json derives its field from
the same exit code the process returns), its kind, and the whole
payload, so the verification family's structured fields reach
consumers on the exception's payload even though the typed surface
carries only the kind string. When no payload exists, the SDK raises
with the process's stderr text as the message, the process code, no
kind, and an empty payload, the exact path S1.2's probe exercised
twice against the 0.1.0 binary. Two shapes are recorded and
dispositioned. First, kinds exist only for the verification family,
because error_json defines them only there, that being INV-CLI-2's
own scope, and every other CLI error reaches the SDK as message and
exit code alone. The taxonomy is the CLI's, and the SDK mirrors it
rather than inventing one, so no mapping defect stands. Second, a run
taken with the check option raises through raise_for_failures, which
joins the task failures into the exception message and leaves the
payload empty, so the structured failures stand available only
through the unchecked call's result. The information is not lost, it
is de-structured, and the shape is the documented bargain of the
check option, accepted here with S1.4's fault suite charged to pin
it. Disposition: no finding, and the payload-less fallback this entry
describes is confirmed as the site where S1.2's deferred diagnostic
remedy attaches in S1.4.

## S1.4: Fault-path depth and the Python floor

Closed 2026-11-26. This slice built three things. First, the fault
suite at python/tests/test_sdk_faults.py, covering the public
functions per fixture in the adopted form, with the coverage stated
exactly. A missing binary raises FileNotFoundError from all nine
public functions including find_binary itself. The seven
process-backed functions that take their result from _invoke_json
are covered against every other fixture: a failing binary (the
Python interpreter standing in as an executable that runs and
refuses the CLI's arguments without a payload, with two stubs
pinning exit codes 1 and 3 exactly) raises VerirunError with the
fixture's exit code and no kind, malformed payloads (a non-JSON
reply and a JSON reply that is not an object, each at a successful
exit) raise the not-a-JSON-object error, and a wrong-version binary
raises VerirunError at exit code 3. The remaining two functions are
dispositioned in words: find_binary never executes the binary it
resolves, so it has no failing or wrong-version mode to test, and
version is fault-tested against the always-failing stub, the one
fixture that can fail a --version call. Second, S1.2's deferred
remedy landed with the suite. The wrong-version fixture is a
generated stub that mimics the 0.1.0 CLI's surface exactly where
S1.2 probed it, answering --version as 0.1.0, rejecting pin as an
unrecognized subcommand, and rejecting the rest without a payload,
and _invoke_json's payload-less path now probes the resolved binary
and names its version beside the SDK's own in the exception message
when the binary answers in the CLI's version form. The pin and run
fault tests assert both version strings, so the diagnostic is pinned
by tests that fail without it. For the five functions S1.2 never
probed against the real binary, the stub proves the SDK's uniform
handling of the payload-less exit 3 shape, not the real binary's
behaviour on those calls. The stub stands in for the real 0.1.0
binary deliberately: it is hermetic and fast, it mimics the two
failure shapes S1.2 proved against the real binary, and the real
cross-version proof already stands in S1.2's probe. The probe costs
one extra --version execution, bounded at ten seconds, on
payload-less failures only. Third, the Python floor is owned by a
hosted leg: a python-floor CI job runs the full SDK suite under
Python 3.10, the oldest version the package claims, so the
classifiers stand as a tested claim rather than an aspiration.
S1.3's recorded shape is pinned by two check-mode tests in the
end-to-end suite, which assert the raised exception carries
execution failures in its message, falls back to its generic text
for a plain exit-code failure, and keeps an empty payload either
way. The workstream closure reviews ran over this slice: the red
team found one should-fix, a version probe that could itself throw
on non-UTF-8 output, repaired here with a discriminating test, and
the spec and standards axes charged the coverage wording this entry
now states exactly. Disposition: SF-S1-1 is landed, the SDK's fault
surface is tested per function as this entry states it, and the
claimed Python range has a hosted floor. Workstream S1 stands
complete with this slice.
