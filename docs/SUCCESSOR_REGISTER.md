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

- S2.0 Published-state ground truth. CLOSED 2026-11-26, on the evidence
  recorded in its section below. Seated first, on Main's sequencing
  note, since the later slices of S1 and S2 reason from what stands
  published.
- S1.1 The contract read. CLOSED 2026-11-26, no finding, on the
  correspondence recorded in its section below.
- S1.2 Binary discovery. CLOSED 2026-11-26, one finding raised and
  deferred to S1.4 with its remedy, on the probe evidence recorded in
  its section below.
- S1.3 Error mapping. CLOSED 2026-11-26, no finding, on the mapping
  recorded in its section below.
- S1.4 Fault-path depth and the Python floor. Queued, and charged by
  S1.2 with the version-mismatch diagnostic remedy recorded in S1.2's
  section.
- S2.1 The publish chain read. Queued.
- S2.2 Shipped-binary verification, per channel. Queued.
- S2.3 Version coherence, extended. Queued.
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
