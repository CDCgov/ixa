# Architectural Decision Records

Architectural Decision Records preserve the reasoning behind significant
architectural and technical choices. Each ADR is a dated historical snapshot:
it describes the problem, constraints, decision, rationale, alternatives, and
consequences as they were understood when the decision was adopted.

ADRs are not maintained as guides to the current implementation. When a
decision changes materially, create a later ADR and connect the records through
their status and supersession metadata. Do not rewrite the earlier ADR to make
it silently describe the new design.

The initial ADRs in this book reconstruct decisions that predate the ADR
process. Their decision dates and traceability are recorded on a best-effort
basis, and uncertainty is stated rather than hidden behind false precision.

ADR identifiers are stable. The initial historical set is numbered in the
best-supported chronological order. Once published, existing ADRs are not
renumbered when an older decision is discovered or a date is refined.

See the [ADR template and authoring guide](template.md) for the flexible
structure used by these records.

| Decision date | Record | Status |
| --- | --- | --- |
| 2026-02-09 | [ADR-0001: Adopt a typed entity and property architecture](0001-adopt-typed-entity-property-architecture.md) | Accepted |
| 2026-02-27 | [ADR-0002: Represent query results with `EntitySet`](0002-represent-query-results-with-entity-set.md) | Accepted |
| 2026-03-17 | [ADR-0003: Maintain property indexes eagerly without `RefCell`](0003-maintain-property-indexes-eagerly.md) | Accepted |
| 2026-06-15 | [ADR-0004: Give each property concrete index ownership](0004-give-properties-concrete-index-ownership.md) | Accepted |
| 2026-06-15 | [ADR-0005: Key indexes by concrete property values](0005-key-indexes-by-concrete-property-values.md) | Accepted |
| 2026-06-15 | [ADR-0006: Restrict `Eq + Hash` to indexable properties](0006-restrict-eq-hash-to-indexable-properties.md) | Accepted |
| 2026-06-26 | [ADR-0007: Introduce composable triggers](0007-introduce-composable-triggers.md) | Accepted |
| 2026-07-07 | [ADR-0008: Define shutdown and shutdown-time plan semantics](0008-define-shutdown-semantics.md) | Accepted |
| 2026-07-10 | [ADR-0009: Distinguish passive plans from liveness-sustaining plans](0009-distinguish-passive-plans.md) | Accepted |
| 2026-07-27 | [ADR-0010: Replace index watermarks with eager new-entity dispatch](0010-replace-index-watermarks-with-eager-dispatch.md) | Accepted |
