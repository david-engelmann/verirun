# Audit register

This register belongs to the post-completion audit programme
chartered in docs/AUDIT_PROGRAMME.md. It records every finding, its
disposition, and the round that owns it. The register is updated in
the same landing as any disposition change, so the copy on main
always describes the world as it stands.

## Round 1. The read

Round 1 read the whole tree line by line at the main head the 0.1.0
publication closed on. The read covered every production source in
the three crates, every test, every benchmark, both examples, the
workspace and crate manifests, the toolchain pin, and all three
workflows. Nothing was changed. The findings below are the round's
output.

Severity is the seat's judgement of user and operator impact if the
finding is exercised. A finding marked breaking changes public
behaviour or a public API, and its entry records the version
consequence under the programme's breaking change rule.

## Correctness

### F1. Stale sandbox outputs leak across runs that share a work root

Major. The run command passes the user named work directory straight
to the scheduler, and the scheduler places each task sandbox at work
root, tasks, task id. The runner creates that directory when absent
and collects the declared outputs from it after a zero exit. Nothing
clears the sandbox and nothing makes it unique to the run. A task
that exits zero without writing a declared output can therefore have
a file left by an earlier run collected and recorded as this run's
artifact, and two verirun processes sharing one work root write into
the same sandboxes. Every test in the suite uses a fresh work root,
so the gate cannot see this. The fix gives each run its own sandbox
root under the work root, after checking whether the architecture
pins the sandbox path shape. The behaviour consequence is that work
roots accumulate one run directory per run until the user removes
them. Not an API break.

Disposition. LAND in Round 2.

### F2. A crafted cache record can panic the coordinator

Minor. The cache lookup in the scheduler checks a stored record's
exec digest, its status, its canonical re-encode, and the hashes of
the blobs it names. It never compares the record's artifact paths
against the task's declared outputs. The work item builder and the
skipped record builder then rely on an expect that a succeeded
producer's record holds its declared outputs. A hand planted store
entry that passes the checks while naming the wrong paths panics the
coordinator thread, which breaks the documented never panics
contract of the run entry point. Reachability needs a crafted store
entry, so the severity is minor. The fix validates artifact paths
during lookup and treats a mismatch as a cache miss.

Disposition. LAND in Round 2.

### F3. Manifest reading accepts structures no run can produce

Minor. Manifest from_value accepts duplicate task ids in one
manifest and duplicate artifact paths in one record. Both survive
canonical re-encode and Merkle root recomputation. Diff then builds
a map keyed by task id, so a shadowed duplicate record silently
drops out of the comparison. The fix rejects duplicates in
from_value. Breaking for hand built manifests that carry duplicates,
which no run produces. The version consequence is a pre-1.0 minor
bump shared with the other breaking findings of this programme.

Disposition. LAND in Round 9. Breaking.

### F4. Canonical encode recursion is unbounded for built values

Minor. The parser caps value depth at 128, but canonical encode
recurses without a bound. A consumer who builds a deep value through
the raw variants can overflow the stack during encode, which is a
crash rather than an error. The fix adds an internal depth guard in
encode mirroring the parser cap, with the public signature
unchanged.

Disposition. LAND in Round 9.

### F5. Duplicate inputs in a task are neither rejected nor deduplicated

Minor. A task that lists the same input path twice frames it twice
into the exec digest and materializes it twice into the sandbox.
Outputs already reject self collision, and depends_on is
deduplicated, so inputs are the odd one out. The fix makes
validation reject duplicate inputs, mirroring outputs. Breaking for
specs that list an input twice, which gains them nothing today. The
version consequence is a pre-1.0 minor bump shared with the other
breaking findings of this programme.

Disposition. LAND in Round 9. Breaking.

### F6. Read loops propagate interrupted errors instead of retrying

Minor. The read loops in put_stream_observed and copy_to propagate
an interrupted error to the caller instead of retrying the read. An
interrupted read is a normal event on a busy host, and today it
fails a put or a materialization that should have succeeded. The fix
retries on the interrupted kind at both sites.

Disposition. LAND in Round 16.

### F7. Directory durability gaps at four sites

Minor. Four gaps share one pattern. The presence pre-check in
put_bytes_observed returns durable on mere presence and trusts a
concurrent put's in-flight durability, leaving a crash window
between that put's rename and its directory sync. The discard path
in finish_put has the same window. finish_put decides the objects
directory sync from a racy is_dir check shared across concurrent
puts. store_cache_entry syncs the by-exec shard directory but never
the by-exec directory itself when it creates the shard. The fix adds
cheap directory syncs at these sites, and the performance round
measures their cost before the pattern spreads further.

Disposition. LAND in Round 16.

### F8. The GC lock has no staleness recovery

Minor. The collector's lock is a create_new lock file removed on
drop, with no staleness recovery. A sweep killed by SIGKILL leaves
the file behind, every later collection fails with LockHeld, and the
error names no remedy. The fix adds age based staleness using the
collector's one clock read, and a LockHeld message that names the
remedy.

Disposition. LAND in Round 16.

### F9. Temp file names can collide after pid reuse

Minor. Temp file names combine the process id, a fixed worker id,
and a process wide atomic counter, created with create_new. After
pid reuse, a name can collide with crash debris from an earlier
process and fail a put spuriously. The failure is safe and never
corrupts, and it needs pid reuse plus the same digest plus surviving
debris. Deferred until a cheaper cross process uniqueness source is
chosen, because the counter alone cannot see across processes.

Disposition. DEFER.

### F10. GC reachability follows pins only

Collection ages objects from file mtime and follows pins only, so an
unpinned manifest's blobs can be collected after the grace period
even when a recent run re-referenced them through the cache, because
repeat puts through the presence pre-check do not refresh mtime.
This is the documented contract of the architecture's collection
section. Pins are the preservation mechanism, and widening
reachability would change what a pin means.

Disposition. REJECT, as contrary to the documented contract.

### F11. Output collection has a walk then open symlink race

Minor. walk_output walks a declared output path component by
component and the collector then opens the file it walked, leaving a
window between the walk and the open. Exploiting the window needs a
live racing grandchild at collection time, and the child already
runs with full host filesystem access, because the sandbox is
hygiene rather than confinement. The walk guards record integrity,
not host secrecy. Deferred, and revisited if direction call D3 or a
confinement feature changes the threat model.

Disposition. DEFER.

## API surface and ergonomics

### F12. copy_to writes unverified bytes before the verdict

Minor. copy_to streams bytes into a caller provided destination and
the digest verdict arrives only at the end, so a failed verification
leaves unverified bytes sitting in the destination. Every caller in
this repository is safe, because materialization aborts the run on
an integrity error. Whether the public contract should promise
discard on error, or state the current behaviour louder in the docs,
is a decision for the API round with the full surface in view.

Disposition. DEFER to Round 3.

## Performance

### F13. Verify check 4 buffers every referenced blob whole

Major. The fourth verification check in the cli library reads every
referenced blob whole through read_to_vec, spread across up to eight
workers since roadmap lane R4. Peak memory during verification is
therefore a multiple of the largest blobs a manifest names, and a
manifest over large artifacts can exhaust a small host during a pure
verification. The scheduler's cache lookup already streams through
copy_to into a sink. The fix streams check 4 the same way. The first
check legitimately needs the manifest bytes themselves and is
unchanged.

Disposition. LAND in Round 4.

### F14. Run planning is quadratic in the graph size

Minor. Run planning calls producer_of once per input, and
producer_of scans every task and every output, and it calls
is_transitive_dependency once per input, which walks the graph each
time. Planning cost therefore grows faster than the graph it plans.
The fix builds the producer map and the ancestor information once
per run, and proves the change with a many task benchmark.

Disposition. LAND in Round 4.

### F15. The runner supervises and drains by polling

Minor. The runner polls try_wait every 2 milliseconds for a task's
whole life, and each pipe drain polls its channel every 5
milliseconds. A long task burns coordinator and drain wakeups for
its entire duration. The fix is a waiter thread and channel design,
or a polling backoff, proven against the timeout and drain suites
that pin the current timing behaviour.

Disposition. LAND in Round 11.

### F16. The parser's duplicate key check is quadratic per object

Minor. The parser checks each new object key against the keys
collected so far, which is quadratic in the number of keys in one
object. Spec and manifest objects are small in every workload the
project has, so the cost is theoretical today. Deferred until a
large object workload exists to measure the fix against.

Disposition. DEFER.

## Structure

### F17. manifest.rs duplicates a pinned constant

Minor. manifest.rs carries a local copy of the maximum safe integer
constant instead of using the definition canonical.rs owns. The
value is pinned by the format, so a drift between the two copies
would be a silent format change. The fix leaves one definition, used
by both modules.

Disposition. LAND in Round 5.

## Error handling

### F18. Verify check 1 misreports store access failures as a missing manifest

Minor. The first verification check maps every store error that is
not an integrity error to ManifestMissing, including read and
permission failures on a manifest that exists. The report sends the
user hunting for a missing file when the real problem is access. The
fix adds a dedicated VerifyError variant for store access failure.
Breaking, because VerifyError is a public enum and exhaustive
matchers gain a variant. The version consequence is a pre-1.0 minor
bump shared with the other breaking findings of this programme.

Disposition. LAND in Round 6. Breaking.

### F19. Pipe read errors are swallowed by the drain

Minor. spawn_pipe_reader swallows pipe read errors, and the drain
finalizes the stream prefix with no signal that the stream errored.
A truncated capture can therefore look complete in the record.
Fixing this needs a report channel change in the runner, which
belongs with the error handling round's enum work. Deferred to that
round.

Disposition. DEFER to Round 6.

### F20. The digest length message counts bytes and calls them characters

Minor. The InvalidLength message in Digest from_hex reports a byte
count while calling it characters. Hex input is ASCII in practice,
so the count agrees until the day it does not. The fix counts
characters, or names the unit honestly in the message.

Disposition. LAND in Round 6.

## Tests

### F21. The distinct put bench measures repeats after its midpoint

Minor. bench_put_throughput in the store bench cycles its payload
seed with a period of 256 across 500 iterations, so the second half
of the run repeats earlier payloads and lands on the presence
pre-check. The bench labelled as distinct puts measures a mix of
full puts and pre-check hits. The fix widens the seed space or cuts
the iteration count below the period.

Disposition. LAND in Round 7.

### F22. Test helper families are duplicated across suites

Minor. The store and cli suites each carry near identical copies of
the temp store helpers, the clear-readonly cleanup walk, and the
fresh root counter. Roughly ten copies exist. A shared test support
module would remove them, but the refactor touches every suite in a
programme that will be editing those suites round by round. Deferred
to the tests round, which will judge whether the churn pays against
the round's other work.

Disposition. DEFER to Round 7.

## Docs and examples

### F23. The release workflow header describes D1 as unsettled

Minor. The release workflow header says crates.io publication stays
a manual act until direction call D1 settles its shape. D1 settled
on 2026-11-16 and 0.1.0 is published, so the comment now misdescribes
the world. The fix rewrites the header to describe publication as it
stands.

Disposition. LAND in Round 8.

## Checked and cleared in Round 1

The read also verified the following surfaces and found nothing to
record. Future rounds start from this list rather than re-litigating
it without new evidence.

- Object equality in value.rs, where sorting both sides by one fixed
  key order is correct because keys are unique.
- Parser grammar strictness, surrogate pairing, and depth semantics,
  with containers capped at 128.
- The canonical ECMA-262 number layout cases and the UTF-16 key sort
  with adjacent duplicate detection.
- Digest framing, where exec inputs bind paths through the input
  digest, env is sorted, and the Merkle tree promotes the odd leaf
  rather than duplicating it.
- The paths rule set, including rejection of drive relative paths.
- The dag validation check order, including the interplay between
  the outputs pre-validation and the collision check.
- Scheduler abort accounting, cache honouring rules, and skipped
  record framing with the empty digest.
- Collector planning outside the lock and sweeping inside it, with
  LockHeld for the loser.
- CLI exit code mapping totality, and show verifying in full before
  it renders anything.
