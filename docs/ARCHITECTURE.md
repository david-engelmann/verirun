# verirun Architecture

Status: Milestone 0 design. This document is the normative design for verirun. No Rust code exists yet. Milestones 1 to 4 implement exactly what is specified here, in stages. Where implementation experience forces a design change, the change lands in this document in the same pull request as the code, with the reason recorded.

Normative words: MUST and MUST NOT state rules an implementation has to follow to claim conformance. SHOULD states a strong default that needs a recorded reason to deviate from.

## 1. What verirun is

verirun is a content-addressed deterministic execution and evaluation runner. A caller describes a directed acyclic graph of tasks in a run file. verirun validates the graph, executes each task in a hermetic subprocess with a cleared environment, captures stdout and stderr as raw bytes, stores every input, output, and log as an immutable blob in a content-addressed store, and emits a manifest that commits to the whole run. The manifest digest is portable. The same graph, the same input bytes, and the same task behaviour produce the same manifest digest on any host, from any workspace directory, with any worker count, at a fixed cache disposition. Cache disposition is part of a record by design: a task served from cache records status `cached` rather than `succeeded`, so a warm-cache run and a cold run of identical work produce manifests that differ in exactly the status fields and the record digests derived from them, and in nothing else. Artifact and log digests are identical across dispositions. INV-RUN-7 states this carve-out and INV-MAN-1 scopes the determinism claim to it.

Four properties carry the design, and every mechanism in this document exists to serve one of them.

1. Canonical hashing. Structured values hash over their RFC 8785 (JCS) canonical bytes, never over a host library serialization.
2. Crash-safe storage. A blob is either fully present and durable, or absent. A crash at any point never leaves a partial blob under a digest name.
3. Hermetic execution. A task sees only the environment pairs its spec declares, plus a small fixed set of defaults. Nothing leaks in from the host shell.
4. Verified manifests. A Merkle root commits to every task record. Changing one byte anywhere in the run changes the root and fails verification with the exact corrupted digest named.

## 2. Crate and module boundaries

The workspace holds three crates. Dependency direction is one way only. Lower crates never depend on higher crates.

### 2.1 verirun-core

The deterministic core. Strictly std-only per DECISIONS.md Q5. No external crates in its dependency tree, in normal or build dependencies. Dev-dependencies for tests and oracles are the only exception, and they never ship in the library.

Rules that bind this crate:

- `#![forbid(unsafe_code)]` at the crate root.
- No filesystem, process, network, clock, or thread-randomness access. The crate is pure computation over bytes and values handed to it.
- No iteration over `HashMap` or `HashSet`. Ordered maps and sets (`BTreeMap`, `BTreeSet`) or sorted `Vec` only.
- SHA-256 is not implemented here and not imported here. The crate defines the digest framing and a `StreamingHasher` trait. Adapter crates supply the concrete hasher. See section 5.4 for the reasoning.
- ECMAScript number formatting is implemented here on top of the standard library shortest round-trip digits. See section 4.3.

Modules:

| Module | Contents |
| --- | --- |
| `value` | The `Value` and `Number` data model. Section 3.1. |
| `parse` | Strict JSON parser over byte slices. Byte offsets on every error. Depth limit. Duplicate key rejection. Section 4.5. |
| `canonical` | RFC 8785 canonical encoder. Writes into any `core::fmt::Write` or byte sink without building an intermediate document string. Section 4. |
| `digest` | `Digest` type, hex codec, domain tags, tuple framing, the `StreamingHasher` trait, execution tuple encoding. Section 5. |
| `cas` | Content-addressed store contracts. Path derivation, temporary file naming as pure functions, the put state machine types, reachability planning for garbage collection. No I/O. Section 6. |
| `dag` | `TaskSpec`, graph validation (Kahn), ready-set scheduling order, output collision checks. Section 7. |
| `paths` | Sandbox relative path validation. Pure checks over path components and declared metadata. Section 8.4. |
| `manifest` | `Manifest`, `TaskRecord`, `ArtifactRecord`, Merkle tree construction and verification planning. Section 9. |
| `error` | All core error types with diagnostic context fields. Section 11. |

### 2.2 verirun-store

The I/O adapter crate. Depends on `verirun-core` and on `sha2` for the concrete SHA-256 implementation. Still `#![forbid(unsafe_code)]`.

Modules:

| Module | Contents |
| --- | --- |
| `sha256` | `Sha256Hasher`, the `StreamingHasher` implementation backed by the `sha2` crate. The only module in the workspace that names a hash implementation. |
| `cas_fs` | The filesystem content-addressed store. The 4-step atomic put protocol, streaming reads, verify-on-read, pin files, the by-exec cache index. Section 6. |
| `runner` | Hermetic subprocess execution. Environment construction, spawn, concurrent pipe drains, timeout enforcement, output collection. Sections 7 and 8. |
| `scheduler` | Worker pool over the validated DAG. Lexicographic ready order, failure propagation, outcome collection into an ordered map. Section 7.3. |
| `gc` | Garbage collection execution. Mark from pins, sweep with grace rules, dry run reporting. Section 10. |
| `clock` | The single place adapter code reads wall-clock time, used only for garbage collection grace decisions and stale temporary file detection. Core crates never import it. Lands with garbage collection in Milestone 3. |

### 2.3 verirun-cli

The binary crate. Produces the `verirun` executable. Depends on `verirun-core` and `verirun-store`, plus `clap` for argument parsing and `serde_json` for run file reading convenience and `--json` output rendering at the process boundary.

Modules:

| Module | Contents |
| --- | --- |
| `main` | Entry point, top level error to exit code mapping. Section 12. |
| `cmd_run` | `verirun run` implementation. |
| `cmd_verify` | `verirun verify` implementation. |
| `cmd_diff` | `verirun diff` implementation. |
| `cmd_gc` | `verirun gc` implementation. |
| `spec_io` | Run file loading. Parses JSON, hands the value to core validation, renders validation errors with file context. |

Boundary rules for the CLI:

- The CLI never canonicalizes, hashes, or validates on its own. It calls core for every decision and store for every side effect.
- Run file parsing in the CLI is a transport concern only. The parsed value is re-checked by core, including duplicate key rejection at the byte level, so a lenient transport parser cannot weaken validation. To keep that guarantee, the CLI reads the run file as bytes and passes the bytes to core `parse` first. `serde_json` is used only for `--json` output, where key order is presentational and never hashed.
- Argument parsing errors exit with code 3, not the clap default of 2. Code 2 is reserved for integrity failures. Section 12.2.

### 2.4 Dependency allowance summary

| Crate | Allowed external crates |
| --- | --- |
| verirun-core | None. Dev-dependencies only: `proptest`, plus test oracles. |
| verirun-store | `sha2`. Dev-dependencies: `proptest`. |
| verirun-cli | `clap`, `serde_json`. |
| Benchmarks | `criterion`, as a dev-dependency of the crate that owns the benchmark. |
| Any crate | `crc32fast` is allowed by DECISIONS.md Q5 but has no planned use. It stays out until a concrete need lands with a design note. |

Every crate root carries `//!` architectural documentation. Lint configuration lives in each `Cargo.toml` under `[lints.clippy]` with `all`, `pedantic`, and `nursery` at deny level, plus `todo` and `unimplemented` denied, and `allow_attributes_without_reason` denied. Any local allow carries a `reason` string.

## 3. Core data model

### 3.1 Values and numbers

```text
Value  = Null | Bool(bool) | Number(Number) | String(String)
       | Array(Vec<Value>) | Object(Vec<(String, Value)>)
Number = a validated f64 payload under the rules of section 4.3
Digest = [u8; 32], displayed as 64 lowercase hex characters
```

Objects store pairs in insertion order in memory. Order in memory carries no meaning. The canonical encoder sorts at encode time, and every equality or hashing path goes through canonical bytes, so two objects with the same pairs in different orders are the same value for all verirun purposes.

`Digest` implements `Ord` by raw byte order. Its hex form is the only text form. Parsing a digest from text accepts lowercase hex only and rejects any other length or character set, with the offending position in the error.

### 3.2 Task specification

A run file is a JSON object with one field, `tasks`, an array of task objects. Each task object has these fields:

| Field | Type | Rule |
| --- | --- | --- |
| `task_id` | string | Non-empty. Unique across the run. Byte order defines sort order everywhere. |
| `executable` | string | Non-empty. Recorded verbatim in the execution digest. Section 8.2 covers resolution. |
| `args` | array of strings | Ordered. May be empty. Order is part of the execution digest. |
| `env` | object of string to string | Declared environment pairs. May be empty. The fixed default keys of section 8.1 MUST NOT appear here. |
| `inputs` | array of strings | Sandbox relative paths of files to materialize from the store before execution. May be empty. |
| `outputs` | array of strings | Sandbox relative paths the task promises to produce. May be empty. Unique across the whole run. |
| `depends_on` | array of strings | Task ids that must succeed first. May be empty. |
| `timeout_ms` | integer or absent | When present, at least 1. Section 8.3. |

Unknown fields in a task object are rejected. A strict schema at the boundary keeps misspellings from turning into silently ignored intent.

### 3.3 Records

```text
ArtifactRecord = (relative_path, content_digest, byte_len u64, is_executable bool)
TaskRecord     = (task_id, exec_digest, status, exit_code Option<i32>,
                  stdout_digest, stderr_digest, artifacts sorted by path)
Manifest       = (format tag, tasks sorted by task_id, merkle_root)
TaskStatus     = Succeeded | Failed | TimedOut | Cached | Skipped
```

`Skipped` marks a task that never started because an ancestor failed or timed out. Its execution digest is still computed from its spec. Its stdout and stderr digests are the blob digest of the empty byte string. Its artifact list is empty and its exit code is absent. A manifest always holds exactly one record per task in the spec, so a manifest covers the whole graph by construction. Because verification re-hashes every blob a manifest names (section 9.3), recording a skipped task obliges the runner to put the empty blob into the store through the normal protocol of section 6.2. The put is idempotent and normally hits the already-present fast path. A manifest that contains skipped records therefore verifies against the store its own run produced, with no special case in the verifier.

## 4. Canonical encoding. The RFC 8785 profile

verirun canonicalization is RFC 8785 (JCS) with a stricter number domain. This section is the full profile. Anything the RFC leaves to the implementation is pinned here.

### 4.1 General rules

- Output is UTF-8 bytes with no whitespace between tokens.
- Object members sort by property name under the UTF-16 rule of section 4.2, recursively at every depth.
- Arrays keep their order.
- Strings encode under the escaping rules of section 4.4.
- Numbers encode under section 4.3. NaN and the infinities have no encoding and are rejected.
- Canonicalization is a streaming write. The encoder takes a sink and never requires the whole output in memory at once.

### 4.2 Key sorting by UTF-16 code units

Property names compare as sequences of UTF-16 code units, produced in Rust by `str::encode_utf16`, compared lexicographically as `u16` sequences. This is the RFC 8785 section 3.2.3 rule. It is not Unicode code point order, and it is not byte order.

The two orders agree except in one case. A name that starts with a supplementary plane character (above U+FFFF) encodes in UTF-16 with a high surrogate in U+D800 to U+DBFF. A name that starts with a character in U+E000 to U+FFFF encodes with that value directly. UTF-16 order places the supplementary name first, because U+D800 sorts below U+E000. Code point order places it last. An implementation that sorts by `char` or by UTF-8 bytes gets this case wrong and produces different bytes, and therefore different digests, for the same logical object. The conformance vectors in section 13 include this exact pair.

Sorting MUST be stable and total. Duplicate property names are not a sorting problem. They are rejected earlier, at parse time, per section 4.5.

### 4.3 Numbers. ECMAScript layout over a strict domain

RFC 8785 section 3.2.2.3 defers number formatting to the ECMAScript `Number::toString` algorithm (ECMA-262). verirun follows that layout exactly, over a restricted domain:

- Negative zero normalizes to zero. The canonical form of -0.0 is `0`.
- NaN, positive infinity, and negative infinity are rejected with `CanonicalError` kind `NonFiniteNumber`, carrying the byte offset when the value came from parsing.
- The safe-integer rule is purely value-based. Any f64 whose value is integral, meaning it has no fractional part, and whose magnitude exceeds 9007199254740991 (2^53 minus 1, the largest IEEE 754 safe integer) is rejected with kind `IntegerOutOfSafeRange`, carrying the value and the bound. The rule looks at the value only, never at the literal form, because the encoder sees values, not source text: a literal-form rule could not be applied consistently by the parser and the encoder. Value-based rejection covers `1e21`, `1.5e21`, and `1e30` alike. No accepted case is lost by this strictness, because every finite f64 with magnitude at or above 2^52 is integral. A non-integral value at that magnitude does not exist, so there is no large non-integral class left to encode.
- A parsed literal that overflows f64 to infinity, such as `1e999`, is rejected at parse time with kind `NumberOutOfRange` and its byte offset.

Layout algorithm, given a finite in-domain f64 value v:

1. If v is zero, emit `0`. This covers both zero signs after normalization.
2. Extract the shortest round-trip decimal digits of the magnitude of v. Rust standard library formatting of f64 produces the shortest digit set that round-trips, but lays it out under Rust rules, never in scientific notation. The encoder parses that decimal text into a digit string `s` of length `k` with no leading or trailing zeros, and an integer `n` such that the value equals `s` times 10 to the power (n minus k), with the first digit of `s` nonzero.
3. Lay the digits out under the ECMA-262 cases:
   - If k is at most n and n is at most 21, emit the digits followed by (n minus k) zeros. No decimal point.
   - If 0 is less than n, n is at most 21, and n is less than k, emit the first n digits, a decimal point, then the remaining digits.
   - If -6 is less than n and n is at most 0, emit `0.`, then (-n) zeros, then the digits.
   - Otherwise emit scientific notation. Emit the first digit, then a decimal point and the remaining digits when k is greater than 1, then `e`, then an explicit sign (`+` for a non-negative exponent, `-` otherwise), then the decimal form of (n minus 1) with no leading zeros.
4. Prefix `-` for negative values.

Worked examples. `1.0` gives `1`. `9007199254740991` gives `9007199254740991`, the largest accepted integer. `9007199254740992` is rejected. `1e20`, `1e21`, `1.5e21`, and `1e30` are all rejected before layout, because each is integral in value and above the bound. `4503599627370495.5` gives `4503599627370495.5`, a non-integral value just below 2^52 with the decimal point inside the digit string. `0.000001` gives `0.000001`. `0.0000001` gives `1e-7`. `333333333.33333329` gives `333333333.3333333` only if that is the shortest round-trip form of the stored f64, which the conformance vectors pin down byte for byte. One consequence of the value-based domain deserves stating plainly: the scientific notation branch of the layout algorithm is reachable for accepted values only at the small end, below 1e-6. The large end of the ECMA-262 layout is specified in step 3 for completeness, and no accepted value can reach it, because every value large enough to need it is integral and already rejected. RFC 8785 Appendix B vectors whose values fall above the safe bound belong to the specified-rejection class of section 13, not to the byte-exact class.

The digit extraction in step 2 is the one place where a subtle bug changes bytes without changing values. The verification plan therefore cross-checks the encoder against two independent oracles in tests: the RFC 8785 Appendix B vectors that fall inside the verirun domain, and a test-side ECMA-262 layout implementation written directly from the specification text, whose digits come from the standard library's scientific formatter, a different code path from the encoder's plain-decimal digit extraction. Section 13.

### 4.4 Strings and escaping

String encoding follows RFC 8785 section 3.2.2.2:

- The quotation mark and the backslash escape as `\"` and `\\`.
- U+0008, U+0009, U+000A, U+000C, and U+000D use the short escapes `\b`, `\t`, `\n`, `\f`, and `\r`.
- Every other control character from U+0000 to U+001F encodes as `\u00xx` with lowercase hex.
- All other characters, including all non-ASCII text, emit as raw UTF-8. Nothing else is escaped. In particular `/` and U+007F emit raw.

Input strings are Rust `String` values, so they are valid Unicode by construction. A lone surrogate can never reach the encoder. The parser rejects unpaired surrogate escapes in source text instead. Section 4.5.

### 4.5 The strict parser

Core `parse` reads a byte slice and produces a `Value`. It is a recursive descent parser with these rules, each carrying the byte offset of the failure:

- Maximum nesting depth 128. Deeper input fails with kind `DepthLimit`. The limit bounds stack use against adversarial input.
- Duplicate object keys fail with kind `DuplicateKey`, carrying the offset of the second key and the key text.
- Numbers follow the JSON grammar strictly. No leading zeros, no leading plus, no bare decimal point. Domain checks from section 4.3 run during the parse, so an out-of-domain number fails at its own offset.
- String escapes follow the JSON grammar. A `\u` escape in the high surrogate range must be followed immediately by a `\u` escape in the low surrogate range, and the pair combines into one character. A lone surrogate of either kind fails with kind `LoneSurrogate`.
- Raw control characters inside strings fail. Trailing bytes after the top level value fail. Empty input fails.
- The parser never panics on malformed input. Every failure path returns a typed error.

## 5. Hashing and domain separation

### 5.1 Digest basics

All digests are SHA-256 outputs, 32 bytes, displayed as lowercase hex. There are no truncated digests anywhere in the system. Two framing patterns cover every digest verirun computes.

Pattern S, for raw byte streams of unknown length:

```text
digest = SHA256( TAG || 0x00 || stream bytes )
```

The stream feeds the hasher in order, in bounded chunks. The total length is never prefixed, because a live pipe stream has no known length at start. Domain separation comes from the tag alone here. That is safe because the outputs of this pattern are fixed 32-byte digests wherever they are composed into larger structures, so no field boundary ambiguity can arise.

Pattern T, for structured tuples, where every field is present before hashing:

```text
digest = SHA256( TAG || 0x00 || field_1 || field_2 || ... )
text field    = u64 little-endian byte length || raw bytes
integer field = fixed width little-endian, width stated per field
digest field  = the raw 32 bytes, no length prefix
bool field    = one byte, 0x00 or 0x01
count field   = u64 little-endian
```

All multi-byte integers in all framing are little-endian. There are no exceptions. Field order is the order listed in the tables below and never changes within a format version.

### 5.2 Domain tags

Every tag is ASCII, starts with the shared prefix `verirun/v1/`, and ends without a trailing slash. The byte 0x00 separates the tag from the payload in both patterns.

| Tag | Pattern | Payload |
| --- | --- | --- |
| `verirun/v1/blob` | S | Raw blob bytes. Files, task inputs, outputs, stdout, and stderr all use this tag, so identical bytes deduplicate to one stored object across all roles. |
| `verirun/v1/input` | T | Relative path (text), then blob digest of the file content. |
| `verirun/v1/env` | T | Pair count, then per pair in sorted order: key (text), value (text). |
| `verirun/v1/exec` | T | Section 5.3. |
| `verirun/v1/artifact` | T | Relative path (text), blob digest, byte length (u64), executable flag (bool). |
| `verirun/v1/task` | T | Task id (text), exec digest, status byte, exit code presence byte plus i32 when present, stdout digest, stderr digest, artifact count, artifact digests in path order. |
| `verirun/v1/node` | T | Left child digest, right child digest. Merkle interior nodes. |
| `verirun/v1/empty` | S | Empty payload. The Merkle root of a manifest with zero tasks. |

Status bytes for the task framing: 0 Succeeded, 1 Failed, 2 TimedOut, 3 Cached, 4 Skipped.

### 5.3 The execution tuple

The execution digest identifies a unit of work independently of any host path. Its fields, in order:

1. Executable string, verbatim from the spec (text).
2. Argument count (count), then each argument in declared order (text each).
3. Environment digest (digest). Computed over the declared pairs plus the fixed defaults of section 8.1, sorted by key bytes then value bytes. The per-task `TMPDIR` value is excluded, because it contains a host path by construction. Section 8.1.
4. Input count (count), then input digests sorted by relative path bytes. Each input digest is the `verirun/v1/input` digest of section 5.2. Sorting by path, not by digest, keeps the framing a direct function of the spec text.
5. Output count (count), then each declared output path in sorted byte order (text each).

Two specs that differ in any of these fields have different execution digests, up to SHA-256 collision resistance. Two specs that differ only in host workspace location, worker count, or `TMPDIR` have the same execution digest. Cache lookup keys on this digest. Section 7.4.

### 5.4 Why the hasher is a trait

The zero-dependency core rule (DECISIONS.md Q5) forbids the `sha2` crate inside `verirun-core`, and the standard library has no SHA-256. The seam has one pinned API shape, decided here so Milestone 1 Stage A writes it literally. It is a boxed trait object created through a factory trait. Generic type parameters over the hasher were considered and rejected: they would infect every public signature in `digest`, `manifest`, and the store contracts, for a saving of one allocation per stream, which is noise next to the I/O each stream performs.

```rust
/// A domain tag. `as_bytes` returns the exact ASCII bytes of section 5.2.
pub enum DomainTag {
    Blob,
    Input,
    Env,
    Exec,
    Artifact,
    Task,
    Node,
    Empty,
}

impl DomainTag {
    pub fn as_bytes(self) -> &'static [u8];
}

/// One streaming hash computation. Implementations are supplied by
/// adapter crates. A hasher is created for exactly one digest and is
/// consumed by `finalize`.
pub trait StreamingHasher: Send {
    fn update(&mut self, bytes: &[u8]);
    fn finalize(self: Box<Self>) -> Digest;
}

/// Creates hashers. Implemented once by `verirun-store` over the `sha2`
/// crate, and by test doubles in core tests. Held as
/// `&dyn HasherFactory` or `Arc<dyn HasherFactory>` at call sites.
pub trait HasherFactory: Send + Sync {
    fn new_hasher(&self, tag: DomainTag) -> Box<dyn StreamingHasher>;
}
```

Rules that bind the seam:

- Core owns framing. A core helper takes a factory and a tag, creates the hasher, and writes the tag bytes plus the 0x00 separator before any payload. Adapter implementations receive a hasher that is already framed and only ever see payload bytes through `update`. No adapter code writes a tag.
- Streaming call sites (pipe drains, CAS puts) hold one `Box<dyn StreamingHasher>` per stream, created before the first byte moves. One-shot helpers in core create, feed, and finalize inside a single function.
- `finalize` consumes the boxed hasher, so a digest cannot be taken twice and a hasher cannot be reused across digests by accident.
- The core test double is a factory whose hashers record every byte they receive. Framing is therefore verified byte for byte in core tests with no real hash function present. The adapter test suite pins the real SHA-256 outputs for fixed inputs, including the empty string and the standard test vectors. A framing bug and a hash bug are separable failures with separate tests.

## 6. The content-addressed store

### 6.1 On-disk layout

```text
<cas>/
  objects/
    <xx>/            first 2 hex characters of the digest, 256 shard dirs
      <digest>       full 64 hex characters, file mode read-only
  tmp/               in-flight puts, same filesystem as objects/
  pins/
    <manifest digest>.pin    empty file, one per pinned manifest
  by-exec/
    <xx>/
      <exec digest>  file holding a task record digest as hex text
  gc.lock            advisory lock file, held only during a sweep
```

Object files hold raw bytes and nothing else. There are no headers, no magic bytes, and no metadata inside an object file. The name is the whole index. Manifests are objects too: a manifest is stored as the blob of its canonical bytes, and its digest is the blob digest of those bytes. The `verirun/v1/empty` style version markers live in the JSON content (the `format` field, section 9.1), not in file framing.

The fan-out prefix is a performance device only. It carries no meaning, and verification never trusts it beyond locating the file.

### 6.2 The 4-step atomic put

Putting a stream into the store is one protocol with two entry shapes:

- Known digest. The caller already hashed the content (for example a file on disk hashed in a first pass). The temporary file name embeds the digest.
- Streamed. The digest is only known at end of stream (for example a live stdout drain). The temporary file name embeds a nonce instead. The protocol joins the known path at step 2, once the stream ends and the digest exists.

Temporary file grammar, inside `<cas>/tmp/`:

```text
<digest>.<pid>.<worker>.<counter>.tmp     known digest puts
stream-<pid>.<worker>.<counter>.tmp       streamed puts
```

`pid` is the process id, `worker` is the worker index or 0 for the main thread, and `counter` is a per-process atomic counter. Uniqueness comes from the tuple, never from clock or randomness. Temporary names never enter any digest.

The four steps, in order, with no step skippable:

1. Streamed write. Create the temporary file with create-new semantics and stream the content through a 64 KiB buffer, feeding the hasher and the file in the same pass. Memory use is bounded by the buffer, independent of blob size.
2. File sync. Call `sync_all` on the temporary file. After this point the bytes are durable under the temporary name.
3. Read-only and rename. Set the file read-only (`Permissions::set_readonly(true)`, mode 0o444 on Unix). Then rename it to `objects/<xx>/<digest>`, creating the shard directory first when absent. Rename within one filesystem is atomic. A reader at the destination path sees the complete old file or the complete new file, never a partial one. When the destination already exists, the put discards its temporary file and reports the digest as already present. Existing content is never rewritten, because the name already commits to the bytes.
4. Directory sync. Call `sync_all` on the destination shard directory. When this put created the shard directory, also call `sync_all` on the `objects/` directory after the rename, so the shard directory entry itself is durable and not only the file entry inside it. Only after these calls return does `put` report success. A success result is a durability claim, and this step is what backs it.

State machine for one put:

```text
Absent -> Writing -> FileSynced -> Renamed -> Durable
              |            |             |
              +------------+-------------+-> Abandoned (temp file left for GC)
```

Crash behaviour at each point:

| Crash point | State after recovery | Handling |
| --- | --- | --- |
| During Writing | Partial file under a temp name. No object. | Caller retries. GC reaps the temp file after the grace threshold. |
| After FileSynced, before rename | Complete file under a temp name. No object. | Same as above. The work is redone, never adopted, because adoption would need a second trust decision. |
| After rename, before directory sync | Object may or may not survive a power loss, depending on the filesystem. `put` had not returned success. | The caller treats the put as failed and retries. A retry finds either a valid object or none. |
| After directory sync | Object durable. | Nothing to do. |

### 6.3 Same-filesystem and aliasing rules

- `tmp/` lives inside the CAS root by construction, so a rename into `objects/` never crosses filesystems and never fails with EXDEV. At store open, the adapter still compares the device ids of `tmp/` and `objects/` (Unix `MetadataExt::dev`) and fails with exit code 3 when they differ. A CAS root assembled from mounts by hand is a configuration error, and it is cheaper to refuse it than to copy across devices silently.
- No hardlinks cross the sandbox boundary in either direction. Sandbox files are copied into the store by streaming, because a hardlinked sandbox file stays mutable through its other name and would break blob immutability. Store blobs are copied out into sandboxes for the same reason in reverse. The read-only mode is a second line of defence, not the mechanism.
- Stored objects are immutable by policy and by mode. Nothing in the system opens an object for writing. Repair of a corrupted object is deletion plus re-derivation from a verified source, never an in-place edit.

### 6.4 Reads and verification on read

- Materializing an input for a task streams the object through the hasher while copying, and compares the result to the requested digest. A mismatch fails the run with exit code 2 before the task starts. Bit rot is caught at use time, not only during audits.
- `verirun verify` re-hashes every object a manifest names, plus the manifest blob itself, and reports the first mismatch with the expected and actual digests. Section 9.3.
- A plain existence check (`contains`) stats the path only. It is used for the cache fast path and carries no integrity claim.

### 6.5 Concurrent writers

Workers put blobs in parallel. Safety rests on three facts. Temporary names are unique per attempt, so writers never share a file. Rename is atomic, so readers never see partial content. And identical digests carry identical bytes by definition of the address, so two writers racing to the same destination write interchangeable content, and the loser discards its copy. The concurrent writer stress test in INVARIANTS.md hammers exactly these claims with barrier-synchronized writers.

## 7. DAG validation and scheduling

### 7.1 Validation, in a fixed order

Validation runs before any task executes, before any sandbox is created. The first failure stops the run with exit code 3. Checks run in this order, so the reported error is deterministic for a given spec:

1. Schema. Every task object matches section 3.2, including the unknown field rule and the `timeout_ms` lower bound.
2. Duplicate task ids. The error names the duplicated id.
3. Unknown dependencies. Every `depends_on` entry names an existing task. The error names the task and the missing id.
4. Colliding outputs. No declared output path appears in two tasks, and no task declares the same output twice. The error names the path and both task ids.
5. Path safety. Every input and output path passes section 8.4.
6. Cycles. Kahn's algorithm (section 7.2). A graph that does not fully order is rejected. The error lists the tasks left with nonzero indegree, sorted.

### 7.2 Kahn ordering

The validator computes indegrees over `BTreeMap` structures and processes a lexicographically ordered ready set. The result is either a full topological order or the sorted set of cyclic remnants. The scheduler reuses the same ready-set discipline at run time, so validation order and execution eligibility can never drift apart.

### 7.3 The scheduler

- Workers: a fixed pool, sized by `--jobs`, defaulting to the available parallelism. Worker count is an execution detail only. It appears in no digest and no record.
- Eligibility: a task becomes ready when all of its dependencies have status Succeeded or Cached. The ready set is ordered by task id. Workers always take the lexicographically smallest ready task. Start order is therefore deterministic, even though finish order is not.
- Failure propagation: when a task ends Failed or TimedOut, every transitive dependent is marked Skipped without starting. Recording a Skipped task puts the empty blob into the store first (section 3.3), so the record's stdout and stderr digests name an object that exists. Independent branches run to completion. The run then finishes and exits with code 1.
- Outcome collection: outcomes land in a `BTreeMap` keyed by task id. The manifest is built from that map in key order. Finish order has no path into the manifest. This is the mechanism behind the worker-count independence invariant.

### 7.4 Cache

Before executing a ready task, the scheduler looks up its execution digest in `by-exec/`. A hit whose task record and output blobs all still exist lets the scheduler reuse the recorded artifacts and mark the task Cached, with the original stdout and stderr digests carried into the new record. A hit with missing blobs is ignored and the task executes. Cache state can therefore only ever cost time, never correctness. After a task succeeds, its execution digest to record digest mapping is written with the same atomic discipline as section 6.2, into the `by-exec/` tree.

## 8. Hermetic execution

### 8.1 Environment

The child process environment is built from nothing, via `env_clear()`, then populated with exactly:

- The task's declared `env` pairs.
- Fixed defaults: `TZ=UTC`, `LC_ALL=C`, `SOURCE_DATE_EPOCH=0`. A spec that tries to declare any of these keys is rejected during validation. Fixed means fixed.
- `TMPDIR`, set by the runner to the task's private temporary directory inside its sandbox. This value is host-specific, so it is excluded from the environment digest (section 5.3) and from the manifest.

There is no `PATH` inheritance. See section 8.2. The effective environment is sorted by key before spawn for stable diagnostics, though spawn order itself is not observable by well-behaved children.

### 8.2 Executable resolution

The spec's `executable` string is recorded verbatim in the execution digest. Resolution at run time works as follows. If the string contains a path separator, it is interpreted relative to the sandbox working directory and must pass section 8.4, which in practice means tasks run executables they brought in as inputs. If it is a bare name, the runner resolves it against the runner's own `PATH` snapshot taken at process start. That snapshot is a host detail and enters no digest. Portability note: a bare name pins intent, not binary identity. Two hosts with different binaries under one name produce records that differ only where their outputs differ, and the manifest shows that difference honestly. A run that needs binary identity pins the executable as an input file instead.

### 8.3 Spawn, drains, and timeout

- The child runs with its working directory set to its sandbox, stdin from null, and stdout and stderr piped.
- Two drain threads start before the runner waits on the child. Each thread streams one pipe through the store's streamed put (section 6.2) in 64 KiB reads. Raw bytes are preserved exactly, including NUL bytes and invalid UTF-8. Nothing is decoded, trimmed, or line buffered anywhere on this path.
- Concurrent draining is a correctness rule, not a performance choice. An OS pipe buffer holds a finite amount, typically 64 KiB. A child that fills the stderr buffer while the runner waits for stdout to close blocks forever. The classic deadlock is designed out by construction here, and the stress tests write far past the pipe capacity on both streams at once.
- Timeout: when `timeout_ms` is present, a supervisor deadline applies from spawn. On expiry the runner kills the child, reaps it with `wait`, and lets the drain threads run to end of stream on the now closed pipes, then joins them. The task record shows TimedOut, the run continues with failure propagation, and the final exit code is 1. No path leaves a drain thread blocked on a dead child's pipe, and no path leaves the child running.
- Exit status: a child exit code is recorded as `exit_code`. Death by signal records exit code as absent and status as Failed, with the signal number in the CLI diagnostic output. Nonzero exit means Failed.

### 8.4 Sandbox and path rules

Each task gets a sandbox directory, `<work>/tasks/<task_id>/`, with subdirectories for inputs, outputs, and the private `TMPDIR`. Declared paths are validated as follows, identically for inputs, outputs, and sandbox relative executables:

1. The path must be relative. Absolute paths are rejected.
2. No component may be `..`. Components are examined with `Path::components`, so tricks with separators or repeated slashes get no special purchase. `.` components are normalized away before any other check.
3. The normalized path must stay inside the sandbox when joined to it. This is a lexical guarantee first.
4. At collection time, the runner walks the declared output path component by component from the sandbox root, calling `symlink_metadata` on every prefix of the joined path, not only on the final path. Any component that is a symbolic link fails the task, whether it is the final component or an intermediate directory, and whatever it points at, including a target inside the sandbox. Checking only the final path would let a symlinked intermediate directory carry the joined path outside the sandbox undetected, so the walk is the rule, not a hardening extra. A missing output marks the task Failed. A non-regular final component (directory, device, fifo) is rejected as an output.
5. Input materialization follows the same component walk on the destination path inside the sandbox input area before writing any bytes, and rejects any symlink component the same way. The runner never follows a symlink out of the sandbox when collecting outputs or materializing inputs, because rules 4 and 5 reject every link component itself before any open through it.

Undeclared files a task writes are ignored. They stay in the sandbox, are never collected, and the sandbox is scratch space the caller owns. Collection covers exactly the declared outputs, in sorted path order, each streamed into the store with its metadata captured as `(relative_path, content_sha256, byte_len, is_executable)`. `is_executable` on Unix is whether any execute bit is set in the mode. No timestamps, user ids, or group ids are captured anywhere. There is nothing time-shaped in an artifact record.

## 9. Manifests and the Merkle tree

### 9.1 Manifest content

A manifest is a JSON object with exactly these fields, encoded as canonical bytes (section 4) and stored as a blob:

```text
format       the string "verirun-manifest/1"
merkle_root  hex digest, section 9.2
tasks        array of task record objects, sorted by task_id, with fields:
               task_id, exec_digest (hex), status (string form),
               exit_code (integer or null),
               stdout_sha256 (hex), stderr_sha256 (hex),
               artifacts: array of { path, sha256 (hex), bytes (u64),
               executable (bool) }, sorted by path
```

Status strings: `succeeded`, `failed`, `timed_out`, `cached`, `skipped`. Byte lengths are JSON numbers and always inside the safe integer domain of section 4.3 by physical necessity. The manifest holds no timestamps, no host paths, no user names, and no tool version string. The `format` field is the version marker. A future format change mints `verirun-manifest/2` and a new domain tag set, and old manifests stay verifiable forever under the v1 rules.

### 9.2 Merkle construction

- Leaves: one per task record, the `verirun/v1/task` digest of section 5.2, ordered by task id.
- Interior nodes: `verirun/v1/node` over the raw left and right child digests, paired adjacently level by level.
- An odd element at any level is promoted unchanged to the next level. It is never duplicated and never hashed with itself. Duplication would let two different leaf counts collide at the root more easily than the design allows.
- A single leaf is its own root. Zero tasks give the `verirun/v1/empty` digest.
- The root is stored in the manifest and is also recomputed by verification from the record fields alone.

Swapping two digest pointers inside a manifest, editing a status, or flipping one byte in any named blob all change either the recomputed root or a recomputed blob digest. Verification compares both, so each tamper class has a dedicated failure path and test.

### 9.3 Verify algorithm

`verirun verify <manifest digest>` performs, in order:

1. Fetch the manifest blob and re-hash it. Mismatch against the requested digest fails with exit code 2.
2. Parse the manifest bytes with the strict parser and re-encode them. Bytes that are not the canonical encoding of their own content fail with exit code 2.
3. Recompute every task record digest and the Merkle root from the fields. A mismatch against the stored `merkle_root` fails with exit code 2 and names the stored and recomputed roots.
4. Collect the set of distinct artifact, stdout, and stderr digests the manifest names, deduplicated by digest across all records, and re-hash each distinct blob exactly once, however many records reference it. The first mismatch fails with exit code 2 and names the expected digest, the actual digest, and one task and path that referenced it.

### 9.4 Diff

`verirun diff <run_a> <run_b>` loads two manifests, verifies both under section 9.3, then compares records by task id. It reports tasks present on one side only, tasks whose status or exit code changed, and tasks whose artifact or log digests changed, in task id order. Exit code is 0 when the runs are equivalent in all compared fields, 1 when they differ, and 2 when either side fails verification. Machine output with `--json` carries the same comparison as structured data.

## 10. Garbage collection

`verirun gc` works in three phases, and `--dry-run` stops after the second:

1. Mark. Read every pin in `pins/`. For each pinned manifest, collect the reachable set: the manifest blob, every artifact and log digest it names, and the by-exec entries its records use.
2. Plan. List `objects/`. An object outside the reachable set is a sweep candidate only when its modification time is older than the grace threshold, set by `--grace-secs` with a default of 3600. Temporary files under `tmp/` follow the same age rule, with no reachability test. The grace rule is the concurrency guard: a blob written by an in-flight run whose manifest is not pinned yet is young, and young blobs are never swept.
3. Sweep. Take `gc.lock` by create-new, failing cleanly when another sweep holds it. Delete the planned files. Report counts and reclaimed bytes. Dry run prints the same plan and deletes nothing.

GC reads wall-clock time for ages. That is why GC planning takes ages as inputs in core and reads the clock only in the adapter (`verirun-store::clock`). Deterministic code never decides anything from a clock. GC itself is a maintenance operation outside the determinism boundary: it changes what is stored, never what a digest means.

## 11. Error model

Every domain error type implements `std::error::Error` and `Display`, and every variant carries the context a person needs to act. A bare variant name is never the whole message.

| Type | Representative context fields |
| --- | --- |
| `CanonicalError` | Kind, byte offset in the source when parsed, the offending key or value text, expected versus actual where a comparison failed. |
| `ParseError` | Byte offset, expected token class, the byte found. |
| `DigestError` | Position of the first invalid hex character, or expected length 64 versus actual. |
| `DagError` | The conflicting terms: duplicated id, missing dependency id with the naming task, colliding path with both task ids, or the sorted cyclic remnant set. |
| `PathError` | The declared path, the rule it broke, the normalized form. |
| `StoreError` | The CAS path, the protocol step in progress (write, sync, rename, directory sync), the OS error. |
| `RunError` | Task id, spawn or drain detail, the timeout budget when a deadline fired. |
| `VerifyError` | Expected digest, actual digest, and the referencing task id and path. Never just `Mismatch`. |

Exit code mapping is total. Every error the CLI can surface maps to exactly one code in section 12.2, and the mapping lives in one function in `verirun-cli::main`.

## 12. CLI surface and exit codes

### 12.1 Commands

```text
verirun run <run-file> --cas <dir> --work <dir> [--jobs <n>] [--seed <u64>] [--json]
verirun verify <manifest-digest> --cas <dir> [--json]
verirun diff <run-a> <run-b> --cas <dir> [--json]
verirun gc --cas <dir> [--grace-secs <u64>] [--dry-run] [--json]
```

`run` prints the manifest digest on stdout on success. Every command prints the active seed to stderr on every run, where the seed comes from `--seed`, else the `SIM_SEED` environment variable, else the default 0. Execution itself does not consume randomness today. The seed governs property runners, simulation tooling, and any future randomized scheduling experiments, and printing it always keeps the reproduction habit in place before it is needed. `--json` renders a structured result object with the same information content as the text output.

### 12.2 Exit codes

| Code | Meaning | Raised by |
| --- | --- | --- |
| 0 | Success, or verified, or (for `diff`) equivalent runs. | Normal completion. `verify` with every check passing. |
| 1 | Task execution failure. | Any task Failed, TimedOut, or Skipped through propagation. A `diff` that finds differences. Spawn failures. |
| 2 | Integrity or tamper verification failure. | Any check in section 9.3. A corrupted blob found during input materialization. |
| 3 | Invalid input. | Schema and DAG validation, canonicalization and parse errors, path rule violations, argument parsing errors, CAS layout misconfiguration such as a cross-device `tmp/`. |

Argument parsing needs one deliberate override. The argument library's default exit code for usage errors is 2, which would collide with the integrity code. The CLI intercepts parse failures and exits 3. There is no path by which a usage error reports as tampering.

## 13. Verification strategy

The full 1-to-1 mapping from invariants to enforcement lives in docs/INVARIANTS.md. This section states the strategy families and the rules that bind all of them.

- Property tests. `proptest` generators build random values, graphs, byte streams, and directory states. Properties assert round trips (parse then encode then parse), order independence (worker counts, insertion orders, finish orders), and framing stability. Case volume comes from the `PROPTEST_CASES` environment variable, 1000 on pull requests and 10000 on main per DECISIONS.md Q6. No test hardcodes a low case count inside its macro.
- Reference vectors. RFC 8785 Appendix B supplies canonicalization vectors. Vectors inside the verirun number domain must reproduce byte for byte. Vectors outside the domain must produce the specified rejection, and the test names which rule rejects them. Number layout is also cross-checked against the independent test-side layout implementation over generated in-domain values, and parse and encode roundtrip properties pin the remaining shape.
- Fault oracles. The tamper suite flips bytes systematically: every byte class of a manifest, one byte in each leaf blob role (artifact, stdout, stderr), and digest pointer swaps inside the manifest structure. Each flip must produce exit code 2 naming the corrupted digest. Crash oracles stop a put at every protocol step boundary, from a fault-injection build used only in tests, and assert the post-crash states of section 6.2.
- Concurrency stress. Barrier-synchronized writers put identical and distinct digests in parallel and assert store contents afterwards by full re-hash. Pipe stress writes well past 64 KiB on stdout and stderr simultaneously, including binary data, under a test deadline so a deadlock fails the test instead of hanging CI.
- Determinism oracles. The same graph runs with 1 worker and with many workers, and from two different workspace directories, and the manifest digests must be equal as bytes. The same seed in two separate processes must produce byte-identical traces from every seeded runner.
- Benchmarks. `criterion` benchmarks cover canonical encoding throughput, streaming hash throughput, and CAS put throughput, each configured with `Throughput::Bytes` and inputs and outputs wrapped in `std::hint::black_box`. CI runs `cargo bench --workspace -- --test` on every pull request, which proves the benchmarks compile and execute. Recorded performance numbers live in the README from Milestone 4. CI does not gate on wall-clock thresholds, because shared runners make such gates flaky. Latency invariants that do gate are stated as bounded behaviour (deadline, memory bound, completion) rather than as timings.
- Allocation discipline. The streaming paths are checked with a counting sink in tests: a writer test double that records call counts and chunk sizes, proving the encoder and the CAS writer stream in bounded chunks and never hand back one whole-blob buffer for large inputs. A global counting allocator is deliberately not used. Installing one needs unsafe code, which this repository forbids in every target, tests included.

## 14. Determinism rules

These rules bind every crate and every future change:

- No `HashMap` or `HashSet` iteration in any code path that can influence bytes, order, or scheduling. Ordered collections only.
- No wall-clock reads and no OS randomness in `verirun-core`. Time enters the system only through `verirun-store::clock`, for GC ages alone.
- No host paths, host names, user ids, timestamps, or process ids in any hashed or manifested content. Process ids appear in temporary file names only, which are never hashed.
- Locale, timezone, and environment of the host process never influence core behaviour. Child environments are built only per section 8.1.
- Floating point work is limited to the number formatting of section 4.3, which is fully specified, and never feeds arithmetic into a digest.

## 15. Rejected alternatives

### A1. Serialize with serde_json and sort the map

Rejected. Three concrete failures. First, key order: the map implementation orders string keys by Unicode code point (byte order), which gives the wrong answer for the supplementary plane case of section 4.2, so digests would diverge from every conformant JCS implementation on exactly the inputs an adversary would choose. Second, duplicate keys: the standard parser keeps the last value and reports no error, which turns a malformed or hostile run file into a different run than the bytes describe. Third, dependency law: DECISIONS.md Q5 keeps the core std-only, and a serde-based encoder cannot live in the core. A hand-rolled encoder over an explicit value model is more code, and it is the only shape that satisfies the ordering rule, the duplicate rule, and the dependency rule at once.

### A2. Read stdout fully, then stderr

Rejected on a failure mode, not on taste. Pipe buffers are finite. A child writing more than the buffer capacity to stderr while the parent drains stdout blocks on a full pipe, while the parent blocks waiting for stdout to close. Neither side can move. The fix costs two threads and bounded buffers, and removes a whole deadlock class. Any single-threaded drain order has the same bug with the streams swapped, and a polling design reintroduces it under timing pressure while adding platform-specific code.

### A3. Hardlink sandbox files into the store

Rejected. A hardlink makes the stored blob and the sandbox file the same inode. The task, or any later process with the sandbox path, can then modify the bytes that a digest names, and the store's core promise dies silently. The read-only mode does not save this, because the owner of the sandbox name can change modes or write through other paths. Copying costs disk bandwidth and buys actual immutability. The symmetric rule for materializing inputs (copy out, never link out) closes the reverse aliasing path, where a task writing through a linked input name would corrupt the blob for every other task.

### A4. Keep the store index in SQLite or an embedded database

Rejected. The index would need a second durability story next to the filesystem one, with its own crash semantics, its own file format, and an external dependency on the core path. The content-addressed layout needs no index at all: the digest is the path, pins are a directory listing, and the cache index is a tree of small files written with the same atomic protocol as blobs. Every state a maintainer must audit is visible with `ls` and `sha256sum`, which matters for a tool whose whole purpose is verifiability. A database would also put opaque binary state where the design currently has none.

### A5. Build the manifest in completion order

Rejected. Completion order depends on machine load, worker count, and pipe timing. A manifest built in that order has a different digest on every run of the same work, which destroys the portability invariant that justifies the project. Collecting outcomes into an ordered map and building once, from sorted records, costs one sort and removes timing from the output entirely.

### A6. Hash with BLAKE3 or another faster hash

Rejected for this design point. SHA-256 is the digest the blueprint fixes, it is what the allowed crate list provides (`sha2`), and its hardware support and universal tooling (`sha256sum` on every machine a maintainer will ever touch) serve the audit story of A4. A faster hash saves time in the one component that benchmarks show is rarely the bottleneck next to process spawn and disk sync, and would cost cross-tool recognition. The domain tag scheme leaves room for a future `verirun/v2/` tag family if the trade ever changes.

## 16. Invariant index

The authoritative statements and their enforcement mapping live in docs/INVARIANTS.md. The families:

- CAN: canonical encoding correctness, key order, number domain, escaping, parser strictness.
- HASH: domain separation and framing unambiguity.
- CAS: atomic put, durability ordering, immutability, concurrent writer safety, same-filesystem guarantee.
- DAG: validation completeness and deterministic first error.
- RUN: hermetic environment, pipe drain liveness, timeout behaviour, deterministic scheduling outcome.
- SBX: path traversal, symlink, and collection rules.
- MAN: manifest determinism, portability, Merkle tamper detection.
- GC: reachability preservation and grace protection.
- CLI: exit code totality and seed reporting.
- LAT: bounded memory streaming and deadlock freedom stated as latency bounds.

## 17. Milestone map

- M0 (this document set): design only. Zero Rust files.
- M1: `canonical`, `parse`, `value`, `digest`, and the filesystem put protocol with concurrent writer tests. Stage A lands interfaces and failing property tests first. Stage B implements to green.
- M2: `dag`, `runner`, `scheduler`, sandbox rules, cache, parallel execution producing manifests.
- M3: the full CLI with exit codes and `--json`.
- M4: tamper suite, crash recovery suite, pipe and timeout stress at volume, benchmarks, README and CONTRIBUTING.

Each milestone closes with a report covering invariants verified, files changed, confirmation that the core is still dependency free, and the exact verification commands with pass counts.
