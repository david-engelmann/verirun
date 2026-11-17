# Post-completion audit programme

This document charters a fifty round audit programme for verirun. The
programme was ordered by the project owner on 2026-11-17, after the
0.1.0 release was published to crates.io and verified from the outside.
It follows the pattern the owner set for the other completed
repositories in the fleet. The programme runs back to back under the
verirun seat, with full decision power inside the seat, until Round 50
closes it.

## Why this programme exists

The build programme and the post-build roadmap are complete. The
crates work, they are published, and they have no real users yet. That
combination is an opportunity that closes on its own. While nobody
depends on the current behaviour, the project can afford full
line-by-line audits that hunt for enhancements, refactorings,
performance improvements, and breaking changes worth making now,
before a 1.0 freeze turns every one of them into a negotiation. The
programme spends that window deliberately instead of letting it pass.

## The seven lenses

Every round works under one lens. The lenses rotate in a fixed order.

1. Correctness. Behaviour against the invariants and the architecture,
   including crash safety, concurrency, and the store's durability
   promises.
2. API surface and ergonomics. The public types, traits, and errors a
   consumer touches, judged for clarity, consistency, and misuse
   resistance.
3. Performance. Hot paths, allocation, memory bounds, and scheduling
   overhead, always proven with before and after measurement in one
   session.
4. Structure. Module shape, duplication, and the seams between core,
   store, and cli, judged against the architecture.
5. Error handling. Whether every failure is reported accurately, at
   the right layer, with a message that names the remedy where one
   exists.
6. Tests. Coverage honesty, oracle strength, duplication across
   suites, and whether each test can fail for a real reason.
7. Docs and examples. Whether the written record describes the world
   as it stands, and whether a new reader can run everything the docs
   claim.

## The round map

Round 1 is the full read and the register. It changes no code. The
register it produces lives in docs/AUDIT_REGISTER.md and opens the
programme.

Rounds 2 through 50 cycle the seven lenses seven times, in the order
above. Round 2 is correctness, Round 3 is API surface and ergonomics,
Round 4 is performance, and so on, with Round 50 closing the programme
under the docs and examples lens. Each lens therefore owns seven
rounds.

A round re-reads its lens surface line by line, works the open
findings owned by its lens in severity order, dispositions anything
new it finds, lands its fixes, updates the register, and reports. A
finding that does not land in its assigned round carries to the next
round of the same lens, and the register says so.

## Dispositions

Every finding carries exactly one disposition.

**LAND.** The finding is accepted. It lands in its assigned round
through the normal gate described below.

**DEFER.** The finding is real but not now. The register records the
reason and the round or the condition that revisits it.

**REJECT.** The finding does not survive scrutiny, or it contradicts
a standing decision or a documented contract. The register records
the reason so the question is not reopened without new evidence.

## How findings land

LAND findings land from Round 2 onward, through the same gate every
change in this repository has used. Work happens on a branch, the
hosted CI runs green on the exact branch head, and the change lands
on main as one commit whose tree is identical to the reviewed branch
tree. One coherent family of fixes lands at a time. The register is
updated in the same landing as any disposition change, so the
register on main always describes the world as it stands.

## Breaking changes

Breaking changes are authorized in principle for the whole programme.
The crates are pre-1.0 and have no downstream users to protect, and
the versioning section of CONTRIBUTING.md already reserves minor
bumps for API breaks in this period. Each breaking finding records
its rationale and its version consequence in the register.

One fence stands. The forever surfaces named in CONTRIBUTING.md are
out of scope for breaking changes under this programme. Digest
framing, canonical encoding, and manifest semantics do not change
here. A change to any of them would be a new domain era, decided
outside this programme, never a silent edit inside an audit round.

## Standing direction calls

Direction calls D2 (the 1.0 freeze date) and D3 (Windows runtime
support) remain the owner's calls. They stand open by design and
block nothing in this programme. Findings that touch them are
dispositioned on their own merits and note the connection.

## Closing the programme

Round 50 closes the programme with a summary record appended to the
register. The summary states what landed, what stayed deferred and
why, and what the fifty rounds leave for the 1.0 decision.
