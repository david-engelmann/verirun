# verirun-platform

`verirun-platform` is the operating system boundary crate of [verirun](https://github.com/david-engelmann/verirun). It isolates the workspace's foreign function calls behind three small safe primitives used by [`verirun-store`](https://crates.io/crates/verirun-store) across Unix and Windows.

## Primitives

- `ProcessGroup` groups a spawned task process tree and terminates every member on timeout, or when the group is dropped while armed. The runner disarms the group once supervision has reaped the leader, so a supervised task's group terminates nothing when it is dropped. On Unix the child leads its own process group and termination sends `SIGKILL` directly to the negative group id through `libc::kill`. On Windows the child is assigned to a job object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and disarming clears that limit.
- `DirHandle` opens path components one at a time relative to a held directory handle while rejecting symbolic links and reparse points at every step. On Unix it uses `openat` with `O_NOFOLLOW`. On Windows it uses `NtCreateFile` with `FILE_OPEN_REPARSE_POINT` and handle attribute inspection, closing the check-then-open race during artifact collection.
- `rename_replace` performs atomic file replacement for store publication. On Windows it invokes `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` and `MOVEFILE_WRITE_THROUGH` while refusing replacement of read-only destinations. On Unix it calls `rename` directly, and the caller completes directory-entry durability with `sync_all` on the parent directory.

## Safety boundary

Every other crate and every test target in the `verirun` workspace carries `#![forbid(unsafe_code)]`. This crate denies `unsafe_op_in_unsafe_fn` and documents the safety invariant on each foreign call site.

## Documentation

- [Architecture specification](https://github.com/david-engelmann/verirun/blob/main/docs/ARCHITECTURE.md)
- [Invariants and traceability matrix](https://github.com/david-engelmann/verirun/blob/main/docs/INVARIANTS.md)
- [API reference on docs.rs](https://docs.rs/verirun-platform)

## Licence

Dual licensed under the [MIT licence](https://github.com/david-engelmann/verirun/blob/main/LICENSE-MIT) and the [Apache License 2.0](https://github.com/david-engelmann/verirun/blob/main/LICENSE-APACHE).
