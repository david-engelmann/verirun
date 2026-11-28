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
- S3.1 The macOS probe. CLOSED 2026-11-27, one platform fact
  recorded, the durability residual and the case fact both
  placed beside PF-1 at closure, and a
  follow-on list scoped, on the probe evidence recorded in its
  section below.
- S3.2 The Windows parity re-proof. CLOSED 2026-11-27, no finding,
  identity re-proved in one pass and the charge's three items
  dispositioned at closure, on the evidence recorded in its
  section below.
- S4.1 The security file and the remaining surfaces. CLOSED
  2026-11-27, two findings landed, one nit landed, and the
  unverifiable claims named, on the evidence recorded in its
  section below.
- S4.2 Case-colliding declared paths refused. CLOSED
  2026-11-27, charged by Main's ruling on the follow-on
  decision S3.1 scoped, on the evidence recorded in its
  section below.
- S4.3 Trailing-colliding declared paths refused. CLOSED
  2026-11-27, charged by Main's ruling at S4 closure as a
  follow-on slice, on the evidence recorded in its section
  below.
- S4.4 Case-colliding task ids refused. CLOSED 2026-11-27,
  charged by Main's ruling at S4 closure as a follow-on
  slice, on the evidence recorded in its section below.

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

## S3.1: The macOS probe

Closed 2026-11-27. The bounded probe list ran hosted on
macos-latest against a binary built from main b1a91396
(diagnostic runs 45994996297 and 45995412810, the second
re-running the case probe inside the annotation cap). Four
probes, four answers.

Temp dir and symlink resolution: the runner's TMPDIR is a
per-user /var/folders path resolving to /private/var/folders,
and /tmp resolves to /private/tmp. A store placed under either
runs the portable spec to the pinned manifest digest exactly,
and the store's object names stand on disk in the lowercase hex
the code constructs. No divergence.

Rename behaviour: renaming over an existing read-only file,
the store's rename-over-existing shape, succeeds on APFS as
on Linux, with the new content in place. No divergence.

BSD tool divergence: the one known case, the wc padding the
identity suite's macOS digest branch already pins, stands as
the recorded instance. The probe raised no new instance, and
the portable spec's own tools, printf and tr, produce the
pinned digest on macOS as elsewhere.

Case handling, the one finding of fact: the hosted volume is
case-insensitive and case-preserving, as APFS is by default. A
spec whose single task declares two outputs differing only by
case runs to success on macOS, and the manifest records both
declared paths with one identical digest, because both writes
landed on the one file the filesystem allows and collection
honestly read what was there. Nothing is falsified, the
manifest matches the collected bytes, and verify stands. But
the same spec on Linux records two distinct artifacts, so a
spec can silently mean different things on different platforms
in a way no step in the run or verify path flags, the identical
digests being indistinguishable from legitimately identical
outputs. This is recorded as a platform
fact of the same family as the wc divergence, not as a store
defect. The charged angle of this probe, case handling against
the store's own lowercase hex object paths, answers cleanly on
the code and the temp probe's listing together: object names
are constructed from lowercase hex digests by the code itself
and every lookup uses a constructed path, never a name taken
from the filesystem, so case-insensitive resolution has no
name of the store's to misresolve.

The follow-on list this slice was charged to scope, for Main
to weigh: one item. A portability guard, either spec validation
rejecting declared input and output paths that differ only by
case, or a documented warning in the same place the docs record
the tool divergence, so the collision is named where a spec
author can see it rather than discovered in a manifest. No
other follow-on is proposed. The temp, rename, and tool probes
came back clean and need none.

Disposition: the probe list is answered in full, the platform
fact stands on the record, and the follow-on is scoped to a
single decision. Workstream S3 stands open on S3.2.

Completion, recorded at the workstream's closure on 2026-11-27:
the closure reviews found the rename probe's durability half
unexamined under the verdict this entry first carried, and the
gap is closed here. A completion probe (run 46015640040)
exercised the sync sequence the store's protocol issues, on
APFS through the operating system's own calls: file fsync,
rename, directory fsync, and a replace followed by resync, all
succeeding with the new content in place. The store issues the
standard Unix sync calls on macOS exactly as on Linux. What
those calls promise differs by platform documentation: macOS
documents fsync as not guaranteeing the storage device's own
cache stands flushed, the stronger request being the
platform's full-sync call, which stands defined on the runner
and which the store does not issue. Power-loss behaviour
itself is not host testable and is not claimed. The residual
is recorded as a platform fact beside PF-1, in PF-1's own
family: durability on this platform is weaker than the call
names suggest, and the mitigation is the same, every read
rehashing, so a lost entry reads as missing and never as
corrupt. The case fact above is placed beside PF-1 by the same
paragraph, and its scope is widened on the record: it is a
case-insensitive-filesystem fact, proven on macOS and expected
on Windows by the same default, and the follow-on item scoped
above covers both. Three verdict bounds are also stated where
they belong: the temp verdict rests on one store run per
location, the rename verdict on the single replace operation
and the sync sequence above, and the tool probe's census is
printf and tr, the two tools the identity suite chose for
byte-identity, so that probe re-proves the suite's premise
rather than hunting divergence. With the durability half
answered, this entry's disposition stands as written.

## S3.2: The Windows parity re-proof

Closed 2026-11-27. The bounded one-pass re-proof ran hosted
once, on all three
platforms, against binaries built from main 965e416f
(diagnostic run 46005448059). The portable spec produced its
pinned digest, c1cd0a900765094f08534d5edb4676a24770c225ca53848de7173a44b2e0815c,
identically on Linux, Windows, and macOS. The
tutorial spec produced its pinned digest,
e63fe409816ad93c944a2043c6c0dba09adf4686ceaf2a1e230c84096223f84a,
on Linux and on
Windows, and the documented macOS variant,
7e0ab658cd3d5c50dd066f4857ea9cf4aa76f9c62a60a7a1deb386ec51584de8,
on macOS, the BSD wc
padding and nothing else, the localization to the wc output
resting on the identity suite's per-task assertions, which pin
both tasks' exec digests and the greet artifact identically
across platforms where this pass harvested manifest digests.
Cross-platform manifest identity,
INV-PLAT-1, stands re-proved on the record in a single pass,
and the one divergence in the frame is the tool's, exactly
where the identity suite's own constants say it lives.

The PF-1 guard, stated at the slice as charged: PF-1,
directory entry durability being weaker on Windows, stands as
an accepted platform fact from the perfection programme. This
slice does not re-open it, re-measure it, or restate its
terms. The re-proof concerns manifest identity only.

Disposition: identity re-proved in one pass, no finding
raised. Workstream S3's slices stand complete, and the
workstream closes under its reviews.

Completion, recorded at the workstream's closure on 2026-11-27:
the closure reviews found this entry, as landed, dispositioned
none of the three items its charge names, the identity pass
above having been executed under a reading of the slice's name
that the charge's text does not carry. The three items are
dispositioned here, and the identity pass stands as additional
evidence under the workstream's framing, not as a substitute
for them.

First, the windows job's full gate, confirmed green at the
programme's start: the hosted record for main 956b705e, the
programme's banking head, is run 45776179849 of 2026-11-26,
completed with the windows job successful, format, clippy, the
full test suite, docs, examples, and benches all green on
windows-latest at the start, and the job has run green on
every gated head since. The confirmation is recorded at
closure from that record.

Second, the SDK's Windows binary discovery, re-proved on
S1.2's evidence pattern (diagnostic runs 46015618613 and
46016036172, windows-latest, the tree's own SDK): with a
placeholder binary at each candidate in turn, the VERIRUN_BIN
override resolved first, the install's scripts directory
resolved next, ahead of the executable's own directory and of
PATH, the executable's directory resolved next, and PATH
resolved last through the system lookup, its returned name
carrying the extension in the lookup's own casing, the same
file on a case-insensitive filesystem. The precedence matches
the code's construction and S1.2's Linux findings tier for
tier.

Third, PF-1 re-checked against the current tree: the platform
seam's files stand byte-identical on current main to their
state across the programme, and read as they stand they say
what PF-1 records. Directory sync on Windows remains the
no-operation variant of the split function, rename remains
MoveFileExW with replace and write-through, and every read
still rehashes. PF-1's terms describe the current tree
exactly. The re-check verifies the recorded fact. It does not
re-open, re-measure, or move it, and any movement needs Main's
ruling under the standing rule.

With the three charged items dispositioned, this slice's
disposition stands as written above.

## S3 closure

Workstream S3 closed 2026-11-27 under the checkpoint rule's
three reviews, and the reviews found this workstream's record
wanting in ways the earlier closures did not. Spec fidelity
returned S3.1 PARTIAL and S3.2 NOT MET as landed: S3.2's entry
dispositioned none of the three items its charge names, the
slice having been designed from the design fold's shorthand
for the charge rather than from the banked roadmap's text, and
S3.1's rename probe carried a durability half it never ran.
The red team graded S3.1's verdicts as broader than their
exercises and its follow-on list as complete only against the
thinner probe set, and S3.2's identity claim as surviving with
its completeness claim partial. Standards found no blockers
and charged the two sections' form: no closure dates, no main
commits, no quoted digests, and one overstated sentence in
S3.1. Every finding was repaired in the closure pass that
carries this section: the three charged items executed and
dispositioned in S3.2's entry, the durability half probed and
dispositioned in S3.1's, both platform facts placed beside
PF-1, the verdict bounds and calibrations stated, and the form
brought up to the register's own shape. The ledger's name for
S3.2 is the roadmap's own, kept as written there, for a slice
whose executed pass covered all three platforms. My own
verification re-read the banked charge against both entries
before repairing them, confirmed the windows job's success in
the programme-start run from the hosted record, and read the
durability seam in the current tree firsthand.

## S4.1: The security file and the remaining surfaces

Closed 2026-11-27. One sweep over the launch surfaces no lane
of this seat had read since they landed, each read against the
current tree with the surfaces' blobs verified unchanged on
main.

SECURITY.md separates into claims this seat can verify and
claims it cannot. Verified against the tree: verify and show
re-hash every referenced object and check recorded artifact
byte lengths and Merkle roots before trusting stored data.
Declared paths are restricted to normalized relative paths
inside the sandbox. Collection walks components through held
handles with O_NOFOLLOW on Unix and FILE_OPEN_REPARSE_POINT
inspection on Windows, the flag standing in the seam under its
NT name. The child environment is the declared pairs, the
three fixed defaults, and the sandbox TMPDIR, and the
frozen-format claim beneath the supported-versions section is
the forever surface this programme has verified in both
directions. Not verifiable from this seat, named as such: the
reporting route, which depends on GitHub private
vulnerability reporting standing enabled on the repository, an
account-side state, and the seven-day initial response, which
is a practice commitment no tree can evidence. One claim did
not survive the read. The threat model said process groups
are terminated on timeout and on drop, written before PR 221
made drop-termination conditional on the group standing armed.
The sentence is repaired in this slice to the landed contract:
terminated on timeout and on drop while supervision is live,
with a supervised task's group disarmed before it is dropped.
Finding SF-S4-1, landed here.

The same imprecision stood on two more surfaces the sweep
read. The platform crate's README said its ProcessGroup
terminates every member on timeout or when the group is
dropped, and the main README promised surviving background
group members are cleaned up when the task finishes. Both are
repaired to the same contract in this slice, the main
README's clause removed rather than reworded, the timeout
claim being the whole of what the runner promises there. The
platform README's remaining claims check out, including its
rename_replace sentence, which compresses the seam's
documented refusal to clear a read-only destination's
attributes before replacing. The core, store, and CLI crate
READMEs read clean in full: the core's module list and its
Pattern S and Pattern T framing are the code's own, the
store's usage example names real methods, and the CLI's
cargo-binstall claim stands on the binstall metadata block in
its manifest, with an override per release target.

dependabot.yml covered Cargo and GitHub Actions and not the
Python home the launch added, whose build requirement is the
tree's one Python dependency. A pip entry in the file's own
shape is added in this slice. Finding SF-S4-2, landed here.

PR 218's text changes, read against the standing copy bar
with the post-scrub text as baseline: the README and the docs
index as they now stand carry no programme talk and no bar
violations beyond the items repaired above, its one changelog
line is ordinary release copy, and its media assets are
outside a copy read as charged. The tamper diagram's alt text
in the README named two domain tags that do not exist, the
framing inverted and a manifest tag invented. The alt text is
repaired to describe the diagram without naming tags. A nit,
landed with the slice.

PR 228 landed while this slice stood staged, on 2026-11-27,
and the slice re-baselined onto its main, e530df2e, as this
roadmap provides for that event. The README was re-read at
the new state: the landing converted relative links to
absolute ones for PyPI rendering and rewrote the Python
quickstart around its new PipelineBuilder, and both repair
sites stood verbatim in the new text, so the repairs above
apply to it unchanged. The landing's other new surfaces, the
PipelineBuilder in the SDK and the verify-published
workflow, stand outside this slice's enumerated list and are
named here for the programme closure's reviews, not absorbed
into a sweep whose charge does not name them.

Disposition: two findings landed, one nit landed, and the
unverifiable claims named rather than chased. Workstream S4
stands complete with this slice, and the programme's slices
stand complete with it.

## S4.2: Case-colliding declared paths refused

Closed 2026-11-27. This slice was charged by Main's ruling of
2026-11-27 on the follow-on decision S3.1 scoped. The ruling
is reject at validation, on the ground that a spec valid on a
case-sensitive filesystem but colliding on a case-insensitive
one is a portability defect of exactly the kind verirun
exists to eliminate, and that documenting the fact alone
would leave the defect reachable. The case fact stays
recorded in S3.1's entry as the rule's reason. S4.1's closing
sentence stood when it was written. This slice, charged the
same day, is the workstream's second, and the programme's
slices stand complete with it. The slice was built on main
e9727ff4, the S4.1 merge, as its coordination claim
provided, and re-based onto main 771bc27e for landing after
PR 258's v0.2.2 preparation took the CHANGELOG head and the
queue refused the first branch on the conflict. Only the
CHANGELOG placement changed in the re-base. The other seven
files are byte-identical to the first branch's versions.

Validation now compares declared paths case-folded as well
as exactly. Check 4 in dag.rs keeps a folded owners map
beside the exact one and returns DagError::CaseCollidingOutput
naming both paths and both tasks. Check 5 keeps a folded map
within each task's inputs and returns
DagError::CaseCollidingInput naming the task and both paths. Folding
is the Unicode lowercase mapping of the whole normalized
path, a pure function of the path, so the refusal is
identical on every platform and a collision through a
directory component alone is caught. Exact duplicates still
report under the older errors, whose comparisons run first.
ARCHITECTURE sections 7.1 and 11 record the comparison, and
INVARIANTS gains INV-DAG-7.

The pin is fault_case_colliding_paths in
crates/verirun-core/tests/dag_faults.rs, with
fault_case_colliding_outputs_exit_3 at the CLI level in
cli_exit_codes.rs. The core cases assert the exact errors
for a cross-task output pair, a same-task output pair, a
directory-only collision, and an input pair, with fold-equal
inputs in different tasks and outputs differing in more than
letter case as controls. Every refusal case validated before
this slice, so the tests fail on the pre-rule behaviour by
construction. The change is breaking for specs that relied
on case-distinct paths and is taken under the pre-1.0
versioning policy, recorded in the CHANGELOG's Unreleased
section.

Checks 4 and 5 moved out of validate_run_spec into their own
functions during the build, because the folded comparisons
carried the function past the workspace's clippy line limit
and the tree carries no allow attributes for that lint. The
extraction changes no behaviour. The local gate ran green
before the push (clippy across the workspace with warnings
denied, fmt, and the dag_faults and cli_exit_codes suites),
with hosted CI as the gate of record.

Disposition: Main's ruling executed as charged. Workstream
S4 stands complete with this slice.

## S4 closure

The workstream's two slices landed as PR 254 (S4.1, CI run
46030187337, main e9727ff4) and PR 260 (S4.2, CI run
46048141832, main 15d9d4e7), with PR 257 (CI run
46039165169) closed unmerged on the queue conflict S4.2's
section records. The three closure reviews ran over the
landed tree at main 15d9d4e7. Spec fidelity returned every
charged item MET on both slices, with one observation.
Standards returned no blockers and no should-fixes, with
seven nits. The red team returned the S4.1 contract claims,
the dependabot claim, and the register claim surviving in
full, and the S4.2 collision claim partially surviving,
with three should-fixes. My own verification concurred with
the lanes' citations, which match the code as built and
locally gated.

Repairs landed with this section: a lowercase sentence
opening in S4.1's section is capitalized, S4.2's section
names the two error variants with their separators and
calls check 5's structure the folded map it is, and
INV-DAG-7's exact-duplicate edge case now says the older
error is reported when it is the violation the checks reach
first in file order. The standards lane's remaining nits
are dispositioned without changes: S4.1's closing sentence
stands reconciled by S4.2's section, the folded-claim
duplication across the two checks is the right call at this
size, and this section supplies the PR and run citations
the S4 sections omitted. The spec lane's observation, that
the docs index rows still carry the word programme, is
dispositioned without a change: the rows name the repo's
own programme documents by title, which the earlier scrub
deliberately kept, and S4.1's sentence is read in that
sense. The red team's contract nits are dispositioned the
same way: the seam's own doc comment carries the Windows
disarm's best-effort disclosure, and the public sentences
state the contract as implemented.

The red team's first should-fix is recorded as a
limitation of the rule as charged and landed. The fold is
the Unicode lowercase mapping, as INV-DAG-7 states, and
the filesystems' own equivalences diverge from it in
corners: the long s against s, final sigma against sigma,
and dotless i against I are one file under the NTFS upcase
table and distinct under the fold, and the filesystems'
tables also disagree with each other, so no single fold
definition closes every corner. Widening the definition is
a spec-visible change, proposed to Main with this closure
rather than taken inside it. The ASCII class and the
common non-ASCII folds are closed as charged.

The red team's other two should-fixes are recorded as
found adjacent defects, outside both slices' charges and
proposed to Main as follow-ons to charge or decline. Win32
strips trailing spaces and dots from each path component,
so two declared paths differing only that way are one file
on Windows while validating as distinct. And task ids are
checked exactly only, so two task ids differing only in
letter case share one sandbox directory on a
case-insensitive filesystem, where the folded comparison
does not reach. Neither contradicts any claim the register
makes. Both are the same one-file defect family in
namespaces the case ruling did not name.

Disposition: Workstream S4 stands complete on the repaired
record, with one limitation recorded and two follow-ons
proposed.

## S4.3: Trailing-colliding declared paths refused

Closed 2026-11-27. This follow-on slice was charged by
Main's ruling at S4 closure, from the S4 red team's second
should-fix: Win32 strips trailing spaces and dots from each
path component when it opens a file, so two declared paths
differing only that way are one file on Windows while
validating as distinct. The ruling charges the refusal at
validation with both paths named, the same defect class as
INV-DAG-7, with a pin that fails on the pre-rule behaviour.

The build: validation checks 4 and 5 gain a third
comparison beside the exact and case-folded ones, keyed on
the case-folded Win32-stripped form of the whole normalized
path, each component with trailing spaces and dots removed.
A stripped hit with a different original path returns
DagError::TrailingCollidingOutput naming both paths and
both tasks, or DagError::TrailingCollidingInput naming the
task and both paths. Keying the stripped form folded means
a pair differing in both letter case and trailing
characters is refused as well. Exact duplicates and pure
case collisions still report first under the older errors.
INV-DAG-8 records the rule, ARCHITECTURE sections 7.1 and
11 describe it, and the CHANGELOG's Unreleased section
carries the breaking note. The pin is
fault_trailing_colliding_paths in dag_faults, whose refusal
cases (a trailing space across tasks, a trailing dot within
one task, a directory-component collision, a combined case
and trailing pair, and a trailing-dot input pair) each
validated before this slice, with
fault_trailing_colliding_outputs_exit_3 pinning exit code 3
and both quoted paths at the CLI. The local gate (clippy
clean, format check, dag_faults and cli_exit_codes green)
and hosted CI are the gate of record.

Disposition: the charged refusal stands landed. The lone
trailing-character path still validates, because the
refusal needs a pair, and that control is pinned in the
same test.

## S4.4: Case-colliding task ids refused

Closed 2026-11-27. This follow-on slice was charged by
Main's ruling at S4 closure, from the S4 red team's third
should-fix: task ids were checked exactly only, so two ids
differing only in letter case share one sandbox directory
on a case-insensitive filesystem, because each task's
sandbox is a directory named by its id under the work root
(scheduler.rs builds work_root/tasks/<task_id> and sandbox
preparation wipes it). The ruling charges the refusal at
validation with both ids named, the same defect class as
INV-DAG-7, with a pin that fails on the pre-rule behaviour.

The build: check 2 keeps a folded map beside the exact set,
and a second id whose Unicode lowercase form matches an
earlier id returns DagError::CaseCollidingTaskId naming
both ids. Exact duplicates still report first as
duplicates. INV-DAG-9 records the rule, ARCHITECTURE
sections 7.1 and 11 describe it, and the CHANGELOG's
Unreleased section carries the breaking note. The pin is
fault_case_colliding_task_ids in dag_faults, whose refusal
case validated before this slice, with
fault_case_colliding_task_ids_exit_3 pinning exit code 3
and both quoted ids at the CLI. The local gate (clippy
clean, format check, dag_faults and cli_exit_codes green)
and hosted CI are the gate of record.

Disposition: the charged refusal stands landed, and with
it the S4 red team's three should-fixes stand fully
dispositioned: the fold limitation recorded and its
widening declined by Main's ruling, and both adjacent
defects charged and landed as S4.3 and this slice.

## Programme closure

Closed 2026-11-27. The successor programme's three
closure reviews ran over main 163007cd, the tree carrying
every slice through the S4 closure repairs, and this
section records their verdicts, the repairs they charged,
and the programme's final state.

Spec fidelity returned every charged item MET across the
twelve banked slices, with the S1, S2 and S3 workstream
closures met as repaired at their closures and the
programme's own closing act the one item the tree could
not yet show. That act is this section.

The red team returned the ledger and tree claims
surviving, and two claims partially surviving. S2.0's
disposition sentence stood unqualified against asymmetric
evidence, and S3.2's completion sentence rested on one
named run. Both are qualified and supported below rather
than rewritten in place. PR 258's surfaces stood unnamed
in the register, and they are named below as standing
unread by this seat.

Standards returned no blockers, two should-fixes of form,
and eight nits. The should-fixes are answered here:
workstream S1's closure verdicts are stated in full
below, and S3.1's disposition sentence is reconciled
below. The nits that asked for text changes are repaired
in this same landing: S3.2's section heading now matches
the roadmap's name, the S4 closure section's closing
disposition is capitalized, and S3.1's ledger line names
both facts placed beside PF-1. The remaining nits (S2.0's
opening form and its disposition inside the slice
narrative, the completion-after-disposition order in
three repaired entries, the ledger's order against the
body's, and finding identifiers carried unevenly) are
dispositioned without changes, as form that does not
misstate any fact.

Workstream S1's closure, stated in full: the red team
approved, with one should-fix on the version probe's
silent failure modes, repaired inside S1.4 itself. Spec
fidelity returned the slice items met in substance with
the coverage form partial, the deviation repaired by
exact restatement in S1.4's entry. Standards approved
with no blockers and no should-fixes, four nits
dispositioned in the same entry. The repairs landed in
PR 238's later heads, and S1.4's entry carries the
closure record in its closing lines.

S3.1's disposition said the workstream stood open on
S3.2. That was true when written. S3.2 closed the same
day, its charged items were discharged at the S3 closure
repairs, and the S3 closure section records the
workstream complete. The sentence stands as written,
reconciled here.

S2.0's disposition said no broken or partial 0.2.0
stands on any channel. The evidence its entry names is
the PyPI install-and-run and the Linux x86_64 archive
check. The crates.io install-and-run of both published
versions stands in S1.2's entry, and the remaining
published archives' functional proof stands in S2.2's
completion. Read with those entries, the disposition is
carried. As written it outran the evidence named beside
it, and this note is the qualification.

S3.2's completion said the windows job has run green on
every gated head since the programme-start run. The
support is collective: every gated PR run the register
cites from S1.1 onward completed SUCCESS, and each of
those runs carries the windows job. The cited runs
include 45907080976, 45914144036, 45919878279,
45937183653, 45952309562, 45961107266, 45969331584,
45986711148, 45996105545, 46006133972, 46017333171,
46030187337, 46048141832 and 46057284203.

Main's rulings at S4 closure stand recorded here as the
programme's final dispositions. The fold widening is
declined: INV-DAG-7's Unicode lowercase mapping stands,
and the NTFS upcase divergence (long s, final sigma,
dotless i) stands as a named limitation, because the
filesystems' tables disagree with each other and no
single fold closes every corner. It reopens only on
concrete Windows deployment evidence. The two adjacent
defects were charged as follow-on slices and landed:
S4.3 as PR 266 on main 18c01bb4, and S4.4 as PR 268 on
main 08a9ca34.

Two landings from David's side stand outside the
programme's slices and unread by this seat, named here
for Main's direction. PR 228 added the Python
PipelineBuilder to the SDK and the verify-published
workflow. PR 258 added the CLI completions subcommand
and the SDK completions surface, the SHA256SUMS
verifying install script, the cargo-deny supply-chain
gate with its deny.toml and CI job, edits to the ci and
stress workflows, and repo furniture (CITATION.cff,
CODE_OF_CONDUCT.md, issue and pull request templates,
Makefile, .editorconfig). Neither landing's substance
has been read by this seat beyond its changelog and file
list.

The open items the programme carries out are S2.1's four
account-side confirmations, which stand for Main or
David: the environment protection rules on the pypi and
crates-io environments, the PyPI trusted-publishing
registration on PyPI's side, the crates.io trusted
publishing state per crate in both its current-state and
direction forms, and release immutability or tag
protection. The programme proposed no version decision
and no freeze, and none is taken here.

Final state: fourteen slice sections stand closed on
evidenced entries, the three closure reviews stand
recorded with every charged repair landed or
dispositioned, and main CI on the final main 08a9ca34
(run 46079369072) completed SUCCESS on all seven
jobs. The successor programme is complete.

## Post-closure repairs

Opened 2026-11-27. The closure section named two
landings from David's side that stood unread by this
seat, PR 228 and PR 258. Main charged a read-only audit
of their surfaces, this seat reported findings with
proposed repairs, and Main approved the four repair
slices as packaged, charged the trailing-colliding
task id refusal below as slice S4.5, and ruled the
audit's env key divergence be closed by tightening the
core to the six keys the SDK and AGENTS.md already
state, as its own slice in the S4 family. Each repair
lands under the three reviews, and each slice records
its entry here as it lands.

## S4.5: Trailing-colliding task ids refused

Closed 2026-11-28. This slice was charged by Main's
ruling on the PR 228 and PR 258 audit, from the audit
red team's adjacent observation: task ids were checked
exactly and by case fold only, so two ids differing
only in trailing spaces or dots name one sandbox
directory on Windows, because Win32 strips those
characters from a path component when it opens a file
and each task's sandbox is a directory named by its id
under the work root. The ruling charges the refusal at
validation with both ids named, the same defect class
as INV-DAG-8 and INV-DAG-9, with pins that fail on the
pre-rule behaviour.

The build: check 2 keeps a stripped map beside the
exact set and the folded map, keyed on the case-folded
Win32-stripped form of the id, and a second id whose
key matches an earlier id returns
DagError::TrailingCollidingTaskId naming both ids.
Exact duplicates still report first as duplicates, and
pure case pairs still report under INV-DAG-9. INV-DAG-10
records the rule, ARCHITECTURE sections 7.1 and 11
describe it, and the CHANGELOG's Unreleased section
carries the breaking note. The pins are
fault_trailing_colliding_task_ids in dag_faults, whose
refusal cases validated before this slice, and
fault_trailing_colliding_task_ids_exit_3 pinning exit
code 3 and both quoted ids at the CLI.

The slice's red team found the same mechanism one
step further in: a lone id made only of spaces and
dots, such as "..." or " ", strips to empty and would
name the tasks directory itself on Windows, and
sandbox preparation wipes the directory it is given.
The repair landed in this slice: is_safe_task_id now
refuses an id whose Win32 stripped form is empty, as
a schema error beside "." and "..", pinned in the
schema rejection table of dag_faults.

Disposition: the charged refusal stands landed, and
the audit's one adjacent observation stands closed.
