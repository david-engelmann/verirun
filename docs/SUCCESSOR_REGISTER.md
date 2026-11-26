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
- S1.1 The contract read. Queued.
- S1.2 Binary discovery. Queued.
- S1.3 Error mapping. Queued.
- S1.4 Fault-path depth and the Python floor. Queued.
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
