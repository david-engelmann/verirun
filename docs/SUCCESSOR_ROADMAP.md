# Verirun successor programme: roadmap

Banked on 2026-11-26 as the successor programme's roadmap, approved in
full by Main Muse the same day after the three design reviews and their
fold. Building follows this banking, with slice S2.0 first, since the
later slices of S1 and S2 reason from what stands published.

## Where the record stands

The perfection programme is complete and closed on the record. Workstream
W (W1 to W7) and rounds P1 to P14 landed, the retroactive closure audit of
PR 211's 0.2.0 tree ran its three reviews, and both approved repair slices
landed (PR 221 at main b6395900, PR 225 and PR 226 at main 956b705e). The
two standing small items from Main's charge stand closed on firsthand
reads recorded by Main. Main CI on the P2 merge c9b9388f is run
45528025000, success on its first attempt, and main CI on the P4 merge
f6a47033 is run 45549558405, success on its first attempt, both confirmed
firsthand on 2026-11-26. What remains open falls into three classes. Some
surfaces landed after the closure audit's tree and were never read by any
review lane. One platform joined the gate without ever receiving a slice
of its own. The accepted items stand on the record as facts rather than
work. The reads below carry their repairs with them, and two slices build
new mechanism rather than only reading.

David's rulings bound this programme throughout. There is no 1.0 freeze
in view and nothing here proposes one. Windows support remains a named
workstream, carried below as a bounded parity re-proof rather than new
building. DECISIONS.md stands exactly as the launch left it. The settled
findings stay settled. PF-2, PF-3, PF-7, and PF-8 stand rejected, F10 and
F22 stand rejected, PF-1 stands accepted as a platform fact, and the
closure audit's nit N-3 (the wall-clock oracle tests) stands accepted as
recorded. None of them re-opens without a ruling from Main.

## Workstream S1: the Python SDK

PR 222 (main 79f3da0b) added a typed Python SDK and PyPI binary wheels,
and PR 223 (main 84e0be13) repaired the release publish job behind them.
No review lane of this seat has read this surface. Its shape at main
956b705e is a wrapper module, python/verirun/__init__.py at 13,672
characters, driving the CLI binary through subprocess and parsing its
--json output into frozen dataclasses. The module exports nine functions
and twelve dataclasses, with the VerirunError exception making thirteen
public classes. Its test suite is two tests in 4,359 characters, run by a
step in the rust, macos, and windows CI jobs. The workstream's gate of
record is hosted CI, with the pypi.yml smoke route as the gate for
wheel-level claims.

Slices:

- S1.1 The contract read. The SDK parses fields the CLI emits, by hand,
  with no shared schema between the two. The slice reads both sides of
  every --json payload against the SDK's parsers and dispositions any
  drift it finds, including fields the SDK silently ignores and fields
  it requires that the CLI does not guarantee. The --json error payloads
  are shared evidence with S1.3, read once and dispositioned once.
- S1.2 Binary discovery. find_binary resolves an explicit argument, then
  the VERIRUN_BIN environment variable, then a search through the Python
  installation's script directories, a source checkout's build trees,
  and PATH, with no version check anywhere. The slice stages the
  experiment the claim needs. The published v0.1.0 release binaries and
  a current 0.2.0 binary stand in one environment under VERIRUN_BIN,
  PATH, and checkout-layout manipulation, and the slice proves which
  binary an SDK of one version drives when a different version is what
  gets found, and whether a version coherence check should exist
  between the wrapper and the binary.
- S1.3 Error mapping. The CLI's exit codes (0 success, 1 task failure,
  2 integrity, 3 schema) map onto VerirunError by hand. The slice checks
  every code and every --json error payload against the mapping,
  including the INV-CLI-2 parity the launch recorded.
- S1.4 Fault-path depth and the Python floor. The existing suite's
  end-to-end test already exercises all nine public functions on the
  happy path, including cache identity and a tamper conviction. The gap
  is fault-path depth, with exactly one negative assertion in the suite
  today. The slice adds one fault test per public function against a
  missing binary, a wrong-version binary, and a failing binary, covering
  exit codes 1 and 3 and malformed payloads. It also settles the
  support range with evidence. pyproject.toml claims Python 3.10
  through 3.14 while CI proves the wrapper only on each runner's own
  Python, so the slice runs the suite on a hosted 3.10 leg or records
  the classifiers as aspirational. The wheel-binary proof belongs to
  S2.2 and is cross-referenced from here, not duplicated.

## Workstream S2: published state, release automation, version coherence

The launch's publish machinery has never had its ground truth
established. PR 223 exists because a live release publish job failed and
needed its context repaired, so a publish was attempted, and no record
this seat holds says what stands published today. The workstream opens
by establishing that state before it reads any automation text. Five
workflows exist (ci.yml, publish.yml, pypi.yml, release.yml,
stress.yml). The perfection programme audited ci.yml line by line across
several rounds. The other four arrived with or after the launch and have
never been read by this seat. The workstream's gate of record is the
hosted workflow runs together with the public indexes, and every build
it needs runs hosted, never on this box.

Slices:

- S2.0 Published-state ground truth. The slice establishes, per channel,
  what stands published under the name verirun. On crates.io, on PyPI,
  and as GitHub Release assets it records published or not, the version,
  the asset inventory, and an install-and-run from the index in a clean
  hosted environment. A broken or partial 0.2.0 on any channel is a
  finding of this slice, on the precedent of the dedicated
  consumer-verification pass 0.1.0 received and 0.2.0 never has.
- S2.1 The publish chain read. publish.yml, pypi.yml, release.yml, and
  stress.yml read as one estate. The publish chain's triggers, jobs, and
  credential routes are read end to end. stress.yml is read with its
  recent run conclusions checked, since a workflow on a weekly cron at
  100,000 property cases is one nobody watches by construction. The read
  scopes itself to what the tree proves. Whether trusted publishing is
  configured on the PyPI project and on each crates.io crate, and what
  protection rules sit on the GitHub pypi environment, is account-side
  state the tree cannot evidence, and the slice names those
  confirmations as items for Main or David in the open.
- S2.2 Shipped-binary verification, per channel. There is no single
  shipped binary. The wheel's binary is built by maturin inside
  pypi.yml with strip enabled, and the GitHub Release archives are built
  separately by release.yml's own build job. The slice verifies each
  channel's binary against its own build inputs, never against the
  other channel. The criterion is manifest identity under the platform
  identity spec together with the version string. Byte-for-byte equality
  between channels is not required and is not claimed. The builds run
  hosted, dispatching pypi.yml with publishing disabled where a wheel is
  needed.
- S2.3 Version coherence, extended. The version lives in three homes,
  the Cargo workspace at 0.2.0, pyproject.toml at 0.2.0, and the SDK's
  __version__ at 0.2.0. release.yml's prepare job already runs a Verify
  crate version parity step over the four crate manifests and their
  inter-crate dependencies against the release tag, so the Rust homes
  are checked at release time. The two Python homes are checked nowhere,
  and the hygiene job (required files, the toolchain pin, and the copy
  bar) has no version check. The slice extends coherence to the two
  Python homes, engaging the existing parity step rather than building
  a second mechanism that could disagree with it about what the version
  is, and weighs whether the check also belongs at pull-request time.

## Workstream S3: platforms, macOS probed and Windows re-proved

The launch added a macos CI job with the full step list, and main CI is
green on it in aggregate. No slice has ever examined macOS semantics
firsthand the way W1 to W7 examined Windows. INV-PLAT-1's three-platform
proof stands in aggregate across the CI legs, as the repaired identity
comment now states plainly. The workstream's gate of record is hosted
evidence on the platform under examination, gathered through macos-latest
and windows-latest diagnostic runs on the pattern the W slices proved.

Slices:

- S3.1 The macOS probe. One bounded slice, not a platform workstream in
  disguise. It runs an enumerated probe list against macOS semantics
  through hosted diagnostic runs and dispositions each probe as
  confirmed, divergent with a repair scoped, or divergent as a recorded
  platform fact beside PF-1. The opening list is APFS case handling
  against the store's lowercase hex object paths, temp directory and
  /tmp symlink resolution under the sandbox protocol, rename and
  durability semantics under APFS against the store's protocol, and BSD
  tool divergences of the kind the tutorial test already carries a
  separate macOS constant for. The slice's second deliverable is the
  scoped list of any follow-on macOS slices its evidence justifies, or
  the recorded judgement that none are needed.
- S3.2 The Windows parity re-proof, bounded. One pass with a defined
  end. The windows job's full gate is confirmed green at the programme's
  start, the SDK's Windows binary discovery is re-proved on S1.2's
  evidence, and PF-1 is re-checked against the current tree. The slice
  closes with one register entry dispositioning all three. PF-1 stands
  as recorded throughout, and any movement on it needs a ruling from
  Main under the standing rule, exactly as every settled finding does.

## Workstream S4: the unreviewed launch surfaces

Three landings after the closure audit's tree were never read by any
lane of this seat. They are PR 218 (the public polish landing), PR 222
(the SDK and wheels, read in S1 and S2), and PR 223 (the release repair,
read in S2). Two PR 211 additions received only the closure audit's
targeted scan. The workstream's gate of record is the tree itself, with
hosted CI carrying any repair the reads produce.

Slices:

- S4.1 The security file and the remaining surfaces, one sweep.
  SECURITY.md is 2,085 characters stating a reporting route, a response
  commitment, and a threat model. The sweep separates its claims into
  the verifiable and the unverifiable from this seat. Its first check
  is named. The threat model says process groups are terminated on
  timeout and on drop, written before PR 221's disarm made
  drop-termination conditional, the same imprecision PR 221's own
  reviews corrected in three other places. The reporting route's
  dependence on GitHub private vulnerability reporting being enabled is
  account-side state, named as such rather than verified. The same
  sweep reads .github/dependabot.yml's coverage against the repo's real
  dependency homes (Cargo, Actions, and now Python), PR 218's text
  changes against the standing copy bar with the post-scrub text as its
  baseline (its media assets are binary and outside a copy read), and
  the four per-crate READMEs given the full docs read the closure
  audit's public-copy scan did not attempt.

## Housekeeping, outside the programme's slices

The local mirror's stale files (runner.rs at zero bytes since the W3
rustfmt incident, the stale local register, the zero-byte Cargo.lock)
are seat-local damage from the write fault. They affect nothing on
GitHub and need no programme slot and no approval, since the restore
touches nothing outside this seat's own working copy. The seat restores
them from current main's blobs through the file tool as ordinary upkeep,
and the roadmap records them only so the item stops travelling as an open
question.

## Shape and closure

This roadmap and its register, docs/SUCCESSOR_REGISTER.md, bank together
before the first slice builds, on the perfection programme's PR 167
precedent. Every finding is dispositioned LAND, DEFER with reason, or
REJECT with reason, and a slice closes only on a register entry carrying
its gate evidence, so no slice closes by paperwork alone. The three
reviews run at each workstream's closure and at programme closure, with
Main free to add checkpoints by charge as it did at W5. Forever surfaces
stay byte for byte what 0.1.0 published. Closure is the four
workstreams' slices each closed on evidenced register entries, final
main CI green on the Linux, macOS, and Windows jobs, and the evidence
returning to Main. The programme proposes no version decision and no
freeze, and it ends where the perfection programme ended, with the
evidence in Main's hands.

## Provenance: the design reviews and their fold

The scope this roadmap banks was drafted by the seat, then tested with
the three design reviews before it went to Main. The red team returned
approve with fixes, carrying two blockers. Its first, that the draft
never established what stands published, became slice S2.0 opening the
release workstream. Its second, that the closure criteria were at once
unreachable and trivial, is answered by S3.2's bounded form and by the
rule that a slice closes only on an evidenced register entry. Its ten
should-fixes are folded at their slices (stress.yml into S2.1, the
existing parity step into S2.3, S1.4 rescoped to fault paths and the
Python floor, S2.2 split per channel with its criterion decided and its
builds hosted, S2.1's account-side limits stated, S3.1 bounded to an
enumerated probe list, S1.2's staging named, SECURITY.md folded into
S4.1 with its claims separated, the Python range owned by S1.4, and the
checkpoints named in the Shape section). The standards axis returned
approve with fixes, eight should-fixes folded likewise (the SDK counts
corrected to nine functions and twelve dataclasses, S3.1 sized as one
bounded slice, S3.2 given an end, the draft's prose colons cleared, the
register's home decided, the wheel proof single-owned by S2.2, a gate of
record named per workstream, and the ambiguous SF-1 reference replaced
by a description in words). The spec fidelity axis returned MET on all
four of its checks, and its five observations are folded (the counts,
S3.2's subordination to the no-reopen rule stated at the slice itself,
S4's three landings enumerated, the hygiene job's required-files check
restored to its description, and the housekeeping section's ungated
status stated plainly). Main approved the folded scope in full on
2026-11-26, with the sequencing note that S2.0 runs first.
