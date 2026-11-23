# Perfection register

This register records the findings of the perfection programme charged on 2026-11-22, when David answered the two open direction calls. D2 gets no 1.0 freeze yet, because verirun is to be perfected first. D3 is answered in favour of building Windows runtime support, as workstream W of the programme. The programme's plan stands in docs/PERFECTION_ROADMAP.md. The audit register is closed and stands as the record of the fifty round audit. This register opens with the audit's three deferred findings carried in, each with its programme assignment, and new findings take PF numbers as the rounds raise them. Every disposition is LAND, DEFER with reason, or REJECT with reason, and this register is updated in the same landing as any disposition change.

## Carried item C1, audit finding F9, assigned to round P1

Temp file names combine the process id, a fixed worker id, and a process wide atomic counter, created with create_new. After pid reuse, a name can collide with crash debris from an earlier process and fail a put spuriously. The failure is safe and never corrupts. The audit deferred it until a cheaper cross process uniqueness source is chosen. The perfection programme takes it in P1, where temp names gain a cross process uniqueness source inside the store adapter, gated by a collision test that fails against the old naming.

## Carried item C2, audit finding F11, assigned to slice W3

Output collection walks a declared output path component by component and then opens the file it walked, leaving a window between the walk and the open. The audit deferred it on the hygiene rather than confinement model, and tied its revisit to direction call D3 or a confinement feature changing the threat model. David's D3 decision fires that trigger. The item is in scope in W3 on the new footing, where collection opens each component relative to a held handle on both platforms and the window closes.

## Carried item C3, audit finding F16, assigned to round P3

The parser checks each new object key against the keys collected so far, which is quadratic in the number of keys in one object. Every object shape the system produces stays under a dozen keys, and the only large object path is a hand crafted manifest blob handed to verify, a local and trusted input. The audit revisited the deferral in its Round 25 and let it stand. The perfection programme decides it on measurement in P3, landing the membership set fix if the common case does not regress and re-justifying the deferral with the measured numbers if it does.

## Findings raised by the programme

None yet. The first perfection round, P1, opens the finding list.

## Workstream records

### W1. The Windows runner core

The runner now compiles and runs on Windows. Process control goes through a new internal crate, verirun-platform, which holds the workspace's only unsafe blocks behind a safe ProcessGroup API. On Unix the group is the mechanism the runner has always used, the child leading its own process group and termination signalling the group through the shell builtin. On Windows the group is a job object created with kill on job close, assigned the child immediately after spawn, and terminated as a whole at the deadline. Executable resolution on Windows applies the PATHEXT snapshot to bare names and treats the executable extensions as the executable test, where Unix uses permission bits. A portable runner test suite proves exit codes, the hermetic environment, byte exact capture, and whole tree termination with the same test text on both platforms, and the Windows CI job runs it. Two consequences are recorded. The platform crate is a fourth workspace crate and a dependency of the store crate, so the next release publishes it alongside the existing three. Assignment immediately after spawn leaves a narrow window in which a child could spawn a process outside the job; the window is the same class the Unix group has always had with session creation, and the drain grace bounds its cost.

The first hosted run of the portable suite failed all four tests, and a staged probe on a throwaway branch isolated the failure below the runner, in the store: the seam attached and terminated cleanly, resolution worked, and execution died inside the put protocol. Two Windows file semantics differences in the put's final step are fixed in this slice because its gate cannot pass without them, and both are recorded here rather than smoothed over. First, the rename at the end of a put raced a concurrent put of the same digest: the presence check passed for both, the winner set the object read-only, and the loser's rename was refused because Windows will not replace a read-only file where Unix replaces identical bytes silently. A failed rename whose destination now exists completes the way the discard path does, syncs and phases included. Second, directory sync has no Windows equivalent, so the step is a no-op there in this slice. That is a real durability difference, and it is carried to W4, whose slice re-proves this protocol under Windows file semantics and writes the mapping down.

With those two fixes in, three of the four portable tests pass on Windows and the runner itself probes clean end to end. The remaining failure is the hermetic environment test's expectation, not the runner: a staged dump of the environment inside a task shows exactly the runner's five pairs, plus three variables `cmd` synthesizes for itself when they are absent (its own path, its extension list, and its prompt). The test now removes those three on Windows before its exact comparison, the same shell bookkeeping that makes the Unix side run the `env` program directly rather than a shell, so the comparison stays exact over everything the runner controls.
