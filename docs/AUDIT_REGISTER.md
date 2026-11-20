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
so the gate cannot see this. The architecture does pin the sandbox
layout, so the shipped fix keeps the layout and attacks the cause.
The runner clears each task's sandbox before the task executes, so
collection only ever sees files the current execution wrote, and
the scheduler holds the work root under a run lock for the run's
duration, so two runs sharing a work root fail loudly instead of
corrupting each other. The new behaviour is pinned as INV-SBX-6 and
INV-RUN-8. Not an API break.

Disposition. LANDED in Round 2.

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
entry, so the severity is minor. The shipped fix validates artifact
paths during lookup and treats a mismatch as a cache miss, and
INV-RUN-7 now states the path condition in its text.

Disposition. LANDED in Round 2.

### F3. Manifest reading accepts structures no run can produce

Minor. Manifest from_value accepts duplicate task ids in one
manifest and duplicate artifact paths in one record. Both survive
canonical re-encode and Merkle root recomputation. Diff then builds
a map keyed by task id, so a shadowed duplicate record silently
drops out of the comparison. The fix rejects duplicates in
from_value. Breaking for hand built manifests that carry duplicates,
which no run produces. The version consequence is a pre-1.0 minor
bump shared with the other breaking findings of this programme.

Disposition. LANDED in Round 9. Breaking. Manifest from_value
rejects a repeated task id after its sort puts duplicates side by
side, and record from_value rejects a repeated artifact path after
construction sorts the artifacts. Two new ManifestError variants
name the duplicates, DuplicateTaskId and DuplicateArtifactPath,
and a new core fault suite pins both rejections plus the
unduplicated controls.

### F4. Canonical encode recursion is unbounded for built values

Minor. The parser caps value depth at 128, but canonical encode
recurses without a bound. A consumer who builds a deep value through
the raw variants can overflow the stack during encode, which is a
crash rather than an error. The fix adds an internal depth guard in
encode mirroring the parser cap, with the public signature
unchanged.

Disposition. LANDED in Round 9. Encoding now threads a depth
counter that enters at 1 exactly as the parser does and rejects a
container sitting past the shared MAX_DEPTH constant from the
parse module, so the values encoding accepts are exactly the
values the parser accepts. The rejection is a new
CanonicalErrorKind named DepthLimitExceeded, and a fault test
pins the boundary on both sides.

### F5. Duplicate inputs in a task are neither rejected nor deduplicated

Minor. A task that lists the same input path twice frames it twice
into the exec digest and materializes it twice into the sandbox.
Outputs already reject self collision, and depends_on is
deduplicated, so inputs are the odd one out. The fix makes
validation reject duplicate inputs, mirroring outputs. Breaking for
specs that list an input twice, which gains them nothing today. The
version consequence is a pre-1.0 minor bump shared with the other
breaking findings of this programme.

Disposition. LANDED in Round 9. Breaking. Validation check 5
now rejects a repeated normalized input within one task with a
new DagError variant named DuplicateInput, after the first
occurrence passes path validation. A fault test pins the
adjacent and separated shapes plus the distinct and fan-in
controls.

### F6. Read loops propagate interrupted errors instead of retrying

Minor. The read loops in put_stream_observed and copy_to propagate
an interrupted error to the caller instead of retrying the read. An
interrupted read is a normal event on a busy host, and today it
fails a put or a materialization that should have succeeded. The fix
retries on the interrupted kind at both sites.

Disposition. LANDED in Round 16. Both read loops now retry on the
interrupted error kind and propagate only real failures, each with
its original step naming. No new test: an interrupted read cannot
be staged through the public store surface, and the put, copy, and
pipe suites pin the behaviour the loops preserve.

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

Disposition. LANDED in Round 16. The presence pre-check and the
discard path in finish_put now sync the shard directory and the
objects directory before reporting the Durable phase, so a
durability claim is always one the reporting call established
itself. finish_put creates the shard with a single create call
whose result, not a racy is_dir check, decides which put owes
the objects sync. store_cache_entry creates its shard the same
way and syncs by-exec itself when its call created the shard,
under a new SyncByExec step. The cost lands on repeat puts and
repeat cache entries, and the next performance round (Round 18)
measures it, as this entry prescribed. No new test: durability
timing across a crash is not observable from a test, and the
crash recovery and concurrency suites pin the protocol the
syncs extend.

Round 18 measurement. The store bench ran interleaved on one
hosted runner, pre-fix tree against fixed tree, two runs each.
Repeat puts measured 24699 and 24760 per second before the
fix, and 3447 and 20662 after it: the two added directory
syncs cost between a little and a lot per put depending on
the disk's flush behaviour, spiky on a shared runner, and
even the worst observed rate stays in the thousands per
second, far above the one put per artifact a run performs.
Distinct puts and verified reads were unchanged within noise
on both sides, as expected, since the syncs touch only the
repeat paths. The measurement closes the charge with no
follow-up change: the syncs are the fix, and the pre-check
still removes the whole-object rewrite it exists to remove.

### F8. The GC lock has no staleness recovery

Minor. The collector's lock is a create_new lock file removed on
drop, with no staleness recovery. A sweep killed by SIGKILL leaves
the file behind, every later collection fails with LockHeld, and the
error names no remedy. The fix adds age based staleness using the
collector's one clock read, and a LockHeld message that names the
remedy.

Disposition. LANDED in Round 16. The lock guard now ages the
standing lock against the collector's one clock read: a lock
older than a day is broken and the create retried once, and a
second refusal means a live collection won the race and stands.
A lock dated in the future reads as fresh. The LockHeld message
names the remedy, matching the run lock's wording. The new
invariant INV-GC-6 pins the behaviour, with two fault tests in
gc_faults (a stale lock broken and the sweep run, a fresh lock
refusing with the remedy in its message). The run lock keeps
its deliberate difference: a run may legitimately hold its
work root for hours, so it has no staleness, and its message
already names the remedy.

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

Round 23 read. The correctness lens passed over the tree again
at the Round 22 state and found nothing new to disposition.
The two standing deferrals were revisited on their own terms:
F9's temp names still fail safe against crash debris after pid
reuse, and F11's walk race still sits inside the hygiene not
confinement model, with D3 parked and no confinement feature
proposed. The Round 16 fixes were re-read in their landed
form: the pre-check syncs both directories before it reports
any phase, so a sync failure claims nothing; the stale lock
retry fires exactly once and a second refusal stands; and the
one race the re-read surfaced, a sweep deleting an aged
unpinned object between the pre-check and its syncs, is the
race the collector's grace period and pin contract already
bound, not a new exposure. The round closes with the read
recorded and no finding raised.

Round 30 read. The correctness lens passed over the tree
again at the Round 29 state and found nothing new to
disposition. No code has changed since the Round 23 read.
The pass re-read the F2 cache validation in its landed
form: the recorded path set is compared against the
declared set with the length clause first, so a duplicate
in the record shrinks the set and trips the guard before
the set comparison runs. The guard the whole cache trust
story rests on is intact. The round closes with the read
recorded and no finding raised.

Round 37 read. The correctness lens passed over the tree
again at the Round 36 state and found nothing new to
disposition. The correctness account is settled: the two
deferrals were revisited in Round 23 and stand on their
recorded reasoning, every other correctness finding has
landed, and no code has changed since Round 30. The round
closes with the read recorded and no finding raised.

## API surface and ergonomics

### F12. copy_to writes unverified bytes before the verdict

Minor. copy_to streams bytes into a caller provided destination and
the digest verdict arrives only at the end, so a failed verification
leaves unverified bytes sitting in the destination. Every caller in
this repository is safe, because materialization aborts the run on
an integrity error. Whether the public contract should promise
discard on error, or state the current behaviour louder in the docs,
is a decision for the API round with the full surface in view.

Disposition. LANDED in Round 3 as the louder statement. The copy_to
documentation now states that verification completes only when the
copy does, that an integrity error may leave some or all of the
object's bytes in the destination, that the caller must discard
them, and that a caller who cannot discard should read through
read_to_vec and write only on success. No behaviour change and no
API change; a discard promise would have cost the streaming shape
that materialization and the scheduler's sink reads rely on. The
Round 3 read of the rest of the API surface under the ergonomics
lens found it consistent and cleared it: Digest carries Display,
Debug, and FromStr beside to_hex and from_hex, the record types are
symmetric in to_value and from_value, the observed variants are
named uniformly across the store, each crate's errors form one enum
with Display and source, and the CLI command functions share one
shape.

### F25. The builders accept what the parsers reject

Minor, found by the Round 10 read. Round 9 made manifest parsing
reject duplicate task ids and duplicate artifact paths, but the
construction paths kept accepting both. Manifest build sorted
the records and computed the root over a duplicated id without
complaint, and record construction sorted a duplicated artifact
path into place, so a built manifest could fail its own round
trip through the value form. The invariant belongs to the types,
not to one door into them.

Disposition. LANDED in Round 10. Both builders now assert the
uniqueness after their sort and panic on a duplicate, with the
panic documented as a programming error in the house assignment
that Manifest digest already uses for its domain. The fallible
builder shape was considered and set aside, because the only
producers are the scheduler over a validated run, where the ids
are unique by validation, and tests. The parse paths keep
reporting the same shapes as errors, and the record parse now
checks before it constructs so the two doors never blur. Fault
tests pin both panics and a built manifest's round trip.

Round 17 read. The API surface lens passed over the tree again
at the Round 16 state and found nothing new to disposition.
The Round 3 cleared list still holds: the digest type carries
its traits, the record types are symmetric in their value
forms, the observed variants are named uniformly, and each
crate's errors form one enum with display and source. The
additions since then follow the same shapes: the run lock
error mirrors the collection lock's fields, the verify
taxonomy's new variants carry the same fields in text and
JSON, the builder panic contract is documented where the
builders live, and the new SyncByExec step names itself after
the Sync family it joins. The round closes with the read
recorded and no finding raised.

Round 24 read. The API surface lens passed over the tree
again at the Round 23 state and found nothing new to
disposition. No public API has changed since the Round 17
read, and the standing mechanical proof still runs every
gate: the examples and benches build and run against the
public surface only, so the surface the repo ships is the
surface its own consumers use. The JSON error shapes the
Round 6 taxonomy added are rendered by a match the compiler
keeps total over the CLI error enum. The round closes with
the read recorded and no finding raised.

Round 31 read. The API surface lens passed over the tree
again at the Round 30 state and found nothing new to
disposition. No public API has changed since the Round 24
read. The documentation floor the surface stands on still
holds mechanically: the workspace denies missing docs, and
no item anywhere carries an allowance against it, so every
public item in the three crates is documented or the gate
fails. The round closes with the read recorded and no
finding raised.

Round 38 read. The API surface lens passed over the tree
again at the Round 37 state and found nothing new to
disposition. The surface has not changed since the Round 3
contract clarification and the Round 10 builder contract,
both of which the later passes keep re-verifying rather
than re-deciding. The round closes with the read recorded
and no finding raised.

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

Disposition. LANDED in Round 4. Check 4 now streams every
referenced blob through copy_to into a sink, in the sequential
pass, the worker chunks, and the panic fallback alike, so a
worker's memory is one stream buffer rather than one whole blob.
The error surface is unchanged because read_to_vec was itself a
copy_to into a vector. No new test pins the change: it is
structural, and the tamper and verification suites pin the
semantics it preserves.

### F14. Run planning is quadratic in the graph size

Minor. Run planning calls producer_of once per input, and
producer_of scans every task and every output, and it calls
is_transitive_dependency once per input, which walks the graph each
time. Planning cost therefore grows faster than the graph it plans.
The fix builds the producer map and the ancestor information once
per run, and proves the change with a many task benchmark.

Disposition. LANDED in Round 4, with the proof amended on the
record. plan_inputs now builds the producer map once and
accumulates every task's transitive dependency set once, in the
dag's topological order, so a dependency's set is final before
any dependent reads it; each input then resolves with two map
lookups, and the per-input graph walk is deleted. The benchmark
clause is amended because planning is an internal step with no
public surface to time: through scheduler::run its cost is
swamped by process spawns at every graph size, so a wall clock
figure would measure the spawns, not the planning. The proof is
the complexity change itself plus a scale test that plans and
runs a chain of a thousand tasks, each consuming its
predecessor's output, and checks the content arrives intact at
the end of the chain.

### F15. The runner supervises and drains by polling

Minor. The runner polls try_wait every 2 milliseconds for a task's
whole life, and each pipe drain polls its channel every 5
milliseconds. A long task burns coordinator and drain wakeups for
its entire duration. The fix is a waiter thread and channel design,
or a polling backoff, proven against the timeout and drain suites
that pin the current timing behaviour.

Disposition. LANDED in Round 11 as the polling backoff. The
supervision wait doubles from 2 milliseconds to a 50 millisecond
cap while the child runs, and never crosses the deadline, so the
timeout still fires on time. The drain channel wait doubles from
5 milliseconds to a 100 millisecond cap while the stream is idle
and resets on every arrival, and the cap is one percent of the
drain grace the cutoff serves, so the extra cut latency is
immaterial. The waiter thread shape was set aside because the
direct child kill fallback cannot cross the thread boundary
without weakening it, and the backoff keeps every timing the
suites pin. No test is added; the pipe and timeout suites are
the proof surface, as the finding itself prescribed.

### F16. The parser's duplicate key check is quadratic per object

Minor. The parser checks each new object key against the keys
collected so far, which is quadratic in the number of keys in one
object. Spec and manifest objects are small in every workload the
project has, so the cost is theoretical today. Deferred until a
large object workload exists to measure the fix against.

Disposition. DEFER.

Round 25 revisit. The performance lens re-examined this
deferral at the Round 24 state and let it stand. Every object
shape the system itself produces stays under a dozen keys:
spec files, task records, artifact records, and the manifest
root are all small by construction, so the quadratic term
never engages on the system's own data. The only large
object path is a hand crafted manifest blob handed to
verify, a local and trusted input. The membership set fix is
available whenever that changes (a set used only for
membership never iterates, so determinism is untouched), but
today it would trade the common case's speed for a case that
does not occur. The deferral stands until an untrusted or
large object workload exists.

Round 32 read. The performance lens passed over the tree
again at the Round 31 state and found nothing new to
disposition. The performance account is fully settled:
every performance finding has landed (the streaming verify,
the linear run planning, the polling backoff), the one
durability cost was measured and accepted in Round 18, and
the one deferral was revisited on its merits in Round 25.
No code has changed since, so there is nothing new to
measure. The round closes with the read recorded and no
finding raised.

Round 39 read. The performance lens passed over the tree
again at the Round 38 state and found nothing new to
disposition. Nothing has changed since the Round 32
settlement of the performance account. The round closes
with the read recorded and no finding raised.

## Structure

### F17. manifest.rs duplicates a pinned constant

Minor. manifest.rs carries a local copy of the maximum safe integer
constant instead of using the definition canonical.rs owns. The
value is pinned by the format, so a drift between the two copies
would be a silent format change. The fix leaves one definition, used
by both modules.

Disposition. LANDED in Round 5. The local constant is deleted and
the byte length check in manifest.rs reads canonical::MAX_SAFE_INTEGER,
the same definition the parser uses. The Round 5 read of the rest
of the tree under the structure lens found no second duplication
of the kind: the store's io_error and sync_directory helpers live
only in cas_fs, and the per-crate error enums are parallel by
design rather than copies.

Round 12 read. The structure lens passed over the tree again at
the Round 11 state and found nothing new to disposition. The
module map still matches the architecture document one concept
per module in the core, one adapter concern per module in the
store, one command module per subcommand in the cli over a
shared library layer, and the crate roots hold no logic. The
largest files are the scheduler and the runner, each a single
cohesive responsibility, and the store helpers still live only
in cas_fs. The round closes with the read recorded and no
finding raised.

Round 19 read. The structure lens passed over the tree again
at the Round 18 state and found nothing new to disposition.
The module map is unchanged since the Round 12 read: core
holds one module per concept, the store one per adapter
concern, the cli one command module per subcommand over a
shared library. The rounds since grew files without
blurring them: the scheduler is 959 lines with the run lock
and the cache entry sync inside the module that owns both,
cas_fs is 568 with the durability work inside the put
protocol it extends, and the new error variants and steps
live in the per-crate error modules beside their families.
No change crossed a boundary it should not have. The round
closes with the read recorded and no finding raised.

Round 26 read. The structure lens passed over the tree again
at the Round 25 state and found nothing new to disposition.
No code has changed since the Round 19 read, and the file
sizes are byte for byte the ones that read recorded, so the
module map and its cohesion conclusions stand as they were.
The programme's own documents keep their structure too: the
charter's rotation and the register's sections still match
the programme being executed. The round closes with the
read recorded and no finding raised.

Round 33 read. The structure lens passed over the tree
again at the Round 32 state and found nothing new to
disposition. The structure has now been stable across three
consecutive passes: no code has changed since Round 19, no
module has moved, and no boundary has been crossed. The
round closes with the read recorded and no finding raised.

Round 40 read. The structure lens passed over the tree
again at the Round 39 state and found nothing new to
disposition. The stability recorded in Round 33 continues:
four passes now without a structural change. The round
closes with the read recorded and no finding raised.

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

Disposition. LANDED in Round 6. Breaking. VerifyError gains
ManifestUnreadable, carrying the digest and the store failure's
own text; check 1 maps only StoreError::Missing to
ManifestMissing and every other non-integrity failure to the new
variant. The CLI's JSON form gains the manifest_unreadable kind
with digest and detail fields, and the exit code stays 2. A new
CLI test denies reads on the manifest object and checks both
forms name access rather than absence.

### F19. Pipe read errors are swallowed by the drain

Minor. spawn_pipe_reader swallows pipe read errors, and the drain
finalizes the stream prefix with no signal that the stream errored.
A truncated capture can therefore look complete in the record.
Fixing this needs a report channel change in the runner, which
belongs with the error handling round's enum work. Deferred to that
round.

Disposition. LANDED in Round 6. The reader channel now carries
io::Result chunks: a read error is forwarded as the channel's
last item, and ChannelReader returns it from read once the
delivered bytes are served, so the drain's put_stream fails and
collect_stream aborts the run with that store error, the same
outcome a dead drain already produced. The cutoff keeps priority
over a forwarded error, so a stream cut at the drain grace after
a timeout still finalizes as an exact prefix. No new test pins
the change: a pipe read error cannot be staged through the
public runner, and the pipe and timeout suites pin the behaviour
around it.

### F20. The digest length message counts bytes and calls them characters

Minor. The InvalidLength message in Digest from_hex reports a byte
count while calling it characters. Hex input is ASCII in practice,
so the count agrees until the day it does not. The fix counts
characters, or names the unit honestly in the message.

Disposition. LANDED in Round 6. The length gate and the message
both count characters now. A digest vector test pins it: a
64-character string with one multibyte character fails as an
invalid character, not a wrong length, and a 63-character string
reports 63.

### F24. Verify check 4 misreports blob access failures as missing blobs

Minor, found by the Round 6 read. The fourth verification check
has the defect F18 names in the first: every store error that is
not an integrity error maps to BlobMissing, including read and
permission failures on a blob that exists. The fix is the sibling
of F18's, landed in the same round so the taxonomy changes once.

Disposition. LANDED in Round 6. Breaking, on the same terms as
F18. VerifyError gains BlobUnreadable, carrying the digest, one
referencing task and path, and the store failure's own text;
check 4 maps only StoreError::Missing to BlobMissing. The JSON
form gains the blob_unreadable kind, and a CLI test denies reads
on an artifact blob and checks both forms.

Round 13 read. The error handling lens passed over the tree
again at the Round 12 state and found nothing new to
disposition. The display chains carry context at every layer,
the store error names its step and path, the run and
collection errors prefix their abort, and the command
functions map store, verify, and collection failures
uniformly, with diff verifying both sides before comparing.
One known gap was confirmed rather than raised again: the
collection lock message names no remedy, where the run lock
message does, and that gap is the second half of F8, which
lands with its staleness design in Round 16. The round
closes with the read recorded and no finding raised.

Round 20 read. The error handling lens passed over the tree
again at the Round 19 state and found nothing new to
disposition. The Round 13 conclusions still hold, and the
Round 16 additions follow them: the collection lock message
now names the same remedy the run lock's message names, in
the same words modulo the lock's owner; the staleness retry
fails conservative, treating an unreadable lock as fresh and
refusing; and SyncByExec reports through the store error
like every other step, named for the directory it syncs.
Exit codes did not move. The round closes with the read
recorded and no finding raised.

Round 27 read. The error handling lens passed over the tree
again at the Round 26 state and found nothing new to
disposition. No error code has changed since the Round 20
read: both error modules are the sizes that read recorded,
and the four verify JSON kinds sit in the renderer exactly
as the Round 6 tests pin them, missing and unreadable kept
distinct for manifests and blobs alike. The round closes
with the read recorded and no finding raised.

Round 34 read. The error handling lens passed over the tree
again at the Round 33 state and found nothing new to
disposition. The error account is fully settled: the Round 6
taxonomy is the last change to it, its two halves were
revisited in Rounds 20 and 27, and nothing has moved since.
The round closes with the read recorded and no finding
raised.

Round 41 read. The error handling lens passed over the tree
again at the Round 40 state and found nothing new to
disposition. Nothing has moved since the Round 34
settlement. The round closes with the read recorded and no
finding raised.

## Tests

### F21. The distinct put bench measures repeats after its midpoint

Minor. bench_put_throughput in the store bench cycles its payload
seed with a period of 256 across 500 iterations, so the second half
of the run repeats earlier payloads and lands on the presence
pre-check. The bench labelled as distinct puts measures a mix of
full puts and pre-check hits. The fix widens the seed space or cuts
the iteration count below the period.

Disposition. LANDED in Round 7. The bench now builds one payload
and stamps the iteration counter into its first bytes, so every
pass puts distinct bytes by construction and the presence
pre-check never fires. The repeat put bench beside it continues to
price the pre-check itself.

### F22. Test helper families are duplicated across suites

Minor. The store and cli suites each carry near identical copies of
the temp store helpers, the clear-readonly cleanup walk, and the
fresh root counter. Roughly ten copies exist. A shared test support
module would remove them, but the refactor touches every suite in a
programme that will be editing those suites round by round. Deferred
to the tests round, which will judge whether the churn pays against
the round's other work.

Disposition. REJECTED at the Round 7 revisit. The revisit counted
eighteen suite files carrying a fresh directory helper and fourteen
carrying a cleanup walk, and read the variants side by side. They
are a family fitted to each file rather than copies of one helper.
The cli helper creates the directory while the store helper only
computes the path the store then opens. One fault suite installs a
panic hook inside its helper. Prefixes, counter orderings, and the
cleanup walks differ in ways the tests carrying them rely on. A
shared module would either grow options until it reassembled the
same variation behind one door, or flatten differences that are
load bearing. The duplication is the honest shape of these suites
and stays.

Round 14 read. The tests lens passed over the suites again at
the Round 13 state and found nothing new to disposition. No
test in the tree is ignored. Every change since Round 7
carries the proof its register entry sanctions: new fault
suites for Rounds 6, 9, and 10, the scale test for Round 4,
and the pipe and timeout suites pinning Round 11's backoff,
which added no test by design. The newest suites assert exact
errors, stderr text, JSON error kinds, and byte exact content;
none is tautological. The three proptest regression files the
local runs created are empty, so no discovered failing case
exists uncommitted. The round closes with the read recorded
and no finding raised.

Round 21 read. The tests lens passed over the tree again at
the Round 20 state and found nothing new to disposition.
Zero ignored tests still, and the regression seed files are
still empty. The only tests added since the Round 14 read
are Round 16's two staleness tests, and both are substantive:
one drives a full sweep through a broken stale lock and
checks the report, the other pins the fresh lock refusal and
the remedy text in its message. Every other round since
carried the proof its register entry prescribed, including
the rounds that prescribed none. The round closes with the
read recorded and no finding raised.

Round 28 read. The tests lens passed over the tree again at
the Round 27 state and found nothing new to disposition.
Nothing in the test tree has changed since the Round 21
read: zero ignored tests, the regression seed files still
empty, and the same 101 test functions plus the property
suites stand behind every gate, including the gates the
replacement landings are re-running on identical trees.
The round closes with the read recorded and no finding
raised.

Round 35 read. The tests lens passed over the tree again at
the Round 34 state and found nothing new to disposition.
The test tree is unchanged since the Round 28 read, and the
suite keeps proving itself the honest way: every
replacement landing re-runs it on an identical tree through
the hosted gate, so its greenness is re-established per
landing rather than inherited. The round closes with the
read recorded and no finding raised.

## Docs and examples

### F23. The release workflow header describes D1 as unsettled

Minor. The release workflow header says crates.io publication stays
a manual act until direction call D1 settles its shape. D1 settled
on 2026-11-16 and 0.1.0 is published, so the comment now misdescribes
the world. The fix rewrites the header to describe publication as it
stands.

Disposition. LANDED in Round 8. The header now states that
publication to crates.io is a separate deliberate act outside the
release workflow, and that 0.1.0 was published that way on
2026-11-16 after D1 settled in favour of publishing.

### F26. The README never points at the audit programme

Minor. The README's Read more list links the architecture
document, the invariant list, the decisions file, the roadmap,
and the tutorial, but not the audit programme charter or its
register, so the programme running over the finished build is
undiscoverable from the repo's front door. The fix adds one
bullet to the list.

Disposition. LANDED in Round 15. The Read more list now carries
the audit programme between the roadmap and the tutorial, one
line naming the 50 round audit and the register that records
every finding and its disposition.

### F27. The show command is missing from the README and the tutorial

Minor. The show command landed in roadmap lane R5, and the
changelog names it, but the README's two summary sentences
still list only run, verify, diff, and collection, and the
tutorial gives every other subcommand a section and show
none. A reader of either document would not learn the command
exists. The fix names show in both README summaries and gives
the tutorial a Look at it section between Verify it and the
cached rerun, with output captured from the shipped program
against the tutorial's own manifest.

Disposition. LANDED in Round 15. The README summaries now name
the show step, and the tutorial's new section prints the real
captured record for its manifest, noting that show verifies
before it displays. The capture was taken on a hosted runner
because the local build was fault blocked; the reproduced run
came out with the tutorial's exact manifest digest, which is
the determinism the tutorial exists to demonstrate.

Round 22 read. The docs and examples lens passed over the
tree again at the Round 21 state and found nothing new to
disposition. The README's audit programme line still says
what is true, the changelog's Unreleased section holds
exactly the rounds that changed behaviour, and the tutorial
and examples stand as the Round 15 pass left them, outputs
and all. The release workflow comment has stayed settled
since Round 8. The round closes with the read recorded and
no finding raised.

Round 29 read. The docs and examples lens passed over the
tree again at the Round 28 state and found nothing new to
disposition. The register itself is now the fastest growing
document in the repo, and its shape still matches the
charter: twenty seven findings under the seven lens
sections in charter order, the Round 1 record at the head,
and the cleared list at the tail. Nothing else in the docs
tree has changed since the Round 22 read. The round closes
with the read recorded and no finding raised.

Round 36 read. The docs and examples lens passed over the
tree again at the Round 35 state and found nothing new to
disposition. Outside this register, the docs tree is
unchanged since the Round 29 read; inside it, each round's
record has been appended in its lens section in order, and
the count of findings stands where Round 15 left it. The
round closes with the read recorded and no finding raised.

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
