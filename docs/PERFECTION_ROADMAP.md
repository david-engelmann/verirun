# Perfection programme roadmap

David decided both open direction calls on 2026-11-22. On D2 there is no 1.0 freeze yet, because verirun is to be perfected first. On D3, Windows runtime support is to be built. This document is the programme those decisions charge. It names two workstreams, the evidence every gate needs, and the criteria the programme closes on. The forever surfaces, the domain tags and digest framing, the canonical encoding rules, and the manifest semantics, stay byte for byte what 0.1.0 published throughout. Any change that would move them is out of scope by construction, and the versioning policy in CONTRIBUTING.md continues to govern everything else.

## Shape of the programme

Two workstreams run under one register and one landing discipline. Workstream W builds the Windows execution backend in seven slices. Workstream P runs perfection rounds in the audit programme's idiom, two full cycles of its seven lenses, hunting enhancements and clearing what the audit deferred. The perfection register records every finding and its disposition, and it is updated in the same landing as any disposition change. Slices and rounds land one at a time through the normal gate, a pull request, hosted CI green, and the landing tree verified against the reviewed tree. The checkpoint rule's three reviews apply at the programme's major checkpoints, which are the W4 gate, the W7 gate, and closure.

Three programme decisions are locked at the outset. First, verirun-core stays strictly std-only, and the platform interfaces the backend needs live in verirun-store, an I/O adapter crate where the locked dependency rules allow external crates. Target gated dependencies, windows-sys on Windows and libc on Unix, are permitted there for the backend and for the F11 fix, each confirmed at its slice gate. Second, Windows code is iterated locally with compiler checks against the Windows target where the build box allows, and proven on hosted Windows runners. Hosted evidence is the gate of record for every Windows claim. Third, the workstream's north star is cross platform manifest identity, stated as a new invariant INV-PLAT-1. The same spec, run on Linux and on Windows, produces the same manifest digest.

## Workstream W, the Windows execution backend

### W1. Windows runner core

Process spawning under a hermetic environment built from nothing, executable resolution under Windows rules, and one job object per task carrying kill on job close, with timeout enforced by terminating the job. The gate evidence is the store crate compiling for Windows with the runner present, the platform fence moved from the whole module to the platform seams, and hosted Windows tests proving spawn, environment isolation, exit code propagation, and a timeout kill that takes the whole spawned tree with it.

### W2. Pipes, draining, and timeout parity

The drain machinery runs on Windows pipes with the same guarantees the POSIX runner pins. Captured stdout and stderr are byte exact, the drain grace after a timeout is the same ten seconds, and a cut stream's digest covers an exact prefix of what the child wrote. The gate evidence is the pipe stress suite passing on Windows at volume, including the full size case with eight mebibytes on each stream held open, and a timeout case whose captured output is an exact prefix.

### W3. Sandbox, output collection, and finding F11

Inputs are materialised into the sandbox, outputs are walked and collected, and the path rules keep one meaning for a spec on both platforms across separators, drive and UNC forms, and case handling. Finding F11 lands in this slice on the new threat model footing that the D3 decision creates. Collection opens each component relative to a held handle, descriptor relative traversal on Unix and handle relative opens on Windows, so the window between the walk and the open closes on both platforms. The gate evidence is the sandbox and output suites passing on Windows, path rule matrix tests passing on both platforms, and a race test that swaps a path component during collection and asserts the collection rejects it.

### W4. Store durability under Windows file semantics

The four step atomic put protocol is re-proved step by step on Windows. File sync maps to FlushFileBuffers, the rename maps to MoveFileEx with write through, read only attribute handling is settled for replace and discard, and directory durability is restated honestly where Windows offers no directory sync equivalent. The gate evidence is the crash recovery and tamper suites passing on Windows, and a durability mapping written into the architecture document that names each POSIX step, its Windows counterpart, and any residual difference in plain words. Any step whose Windows guarantee is weaker than its POSIX counterpart is recorded as a finding, never smoothed over.

### W5. Scheduler parity and the identity proof

The full run path works on Windows, covering planning, the cache, the by-exec records, and manifest construction. The gate evidence is the tutorial spec run on hosted Linux and hosted Windows runners producing the identical manifest digest, with both outputs captured into the perfection register, and INV-PLAT-1 pinned by a CI step that runs the identity check on both platforms.

### W6. Test suites ported

The runner, scheduler, and store suites execute on Windows. The shell script test idiom is replaced by a portable test helper binary that performs the file operations the suites need, so the same suite text runs on both platforms and means the same thing. The gate evidence is the suites running on Windows in CI with their test counts reported in the register, and zero tests ignored on Windows without a recorded reason.

### W7. The Windows CI job grown to the full gate

The Windows job gains the Linux job's steps, format check, clippy with warnings denied, the full workspace test suite, documentation, benches compiled, and examples run, at one thousand property cases on pull requests. The gate evidence is a green Windows job with the same steps as the Linux job on a programme pull request, with the job's runtime recorded in the register so the ten thousand case main policy for Windows can be decided on evidence.

## Workstream P, the perfection rounds

Fourteen rounds, P1 to P14, run two cycles of the seven lenses in the audit programme's order, which is correctness, API surface and ergonomics, performance, structure, error handling, tests, and docs and examples. Every finding is dispositioned LAND, DEFER with reason, or REJECT with reason in the perfection register. Three assignments are named inside the cycles now.

P1, the first correctness round, clears F9. Temp file names gain a cross process uniqueness source inside the store adapter. Its gate needs a collision test that fails against the old naming, with crash debris planted under a reused process id's names, and passes after the change.

P3, the first performance round, decides F16 on measurement. The membership set fix is benchmarked against the current duplicate key check on the parser's real workloads. It lands if the common case does not regress, and the deferral is re-justified with the measured numbers if it does. The same round refreshes the README benchmark figures from a current hosted measurement, replacing the figures carried since the first build.

P5, the first error handling round, retires the known naming wart in the store's stream put path, where one step variant's name says write while it carries source read errors. The rename is free before 1.0 and lands with an honest variant name.

The docs rounds P7 and P14 close each cycle. They include the tutorial and the architecture document's platform sections rewritten for the two platform reality once workstream W has landed. If a cycle closes with a Major finding still open, a further cycle seats automatically. Rounds interleave with W slices as the build slot allows, and neither workstream waits on the other except where a round's subject is a W surface.

## Closure criteria

The programme closes when all of the following stand on main. Slices W1 to W7 landed with their gate evidence recorded in the register. Rounds P1 to P14 landed. F9 and F16 landed or re-justified on the record with measurements. F11 landed in W3. The register carries a closing summary in the audit register's form. Final main CI is green on both the Linux and the Windows jobs. At closure, D2 returns to David with the perfection evidence in hand, and the freeze question is asked again on a perfected repo.
