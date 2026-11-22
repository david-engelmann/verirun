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
