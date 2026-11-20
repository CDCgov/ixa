# ADR template and authoring guide

An ADR should capture one coherent architectural or technical decision at a
useful level. One ADR may draw on several plans, notes, experiments, issues,
pull requests, and discussions. Conversely, one source may contribute to
several distinct ADRs.

The outline below is a guide, not a strict schema. Historical evidence is often
incomplete. Omit unavailable or inapplicable fields and sections rather than
speculating or adding empty boilerplate.

## Suggested outline

````markdown
# ADR-NNNN: Concise decision title

| Field | Value |
| --- | --- |
| Decision date | YYYY-MM-DD, YYYY-MM, YYYY, approximate, or unknown |
| Recorded | YYYY-MM-DD |
| Status | Accepted |
| Supersedes | ADR-NNNN, if applicable |
| Superseded by | ADR-NNNN, if applicable |
| GitHub issues | Links or issue numbers, if known |
| Pull requests | Links or PR numbers, if known |
| Feature branches | Branch names, if useful and known |
| Research | Links to relevant Internals research artifacts |

## Summary

One or two paragraphs stating the decision and why it mattered.

## Context

Describe the problem, constraints, prior design, requirements, and
circumstances that made a decision necessary.

## Decision

State what was chosen. Include the decision's important boundaries,
invariants, and semantics rather than reproducing implementation details.

## Consequences

Record positive outcomes, limitations, risks, operational effects, and
trade-offs known at the time.

## Alternatives considered

Describe credible alternatives that were evaluated and why they were not
selected under the constraints at the time.

## References

Link durable issues, pull requests, commits, research artifacts, and related
ADRs. Include useful feature branch names even if those branches may later be
deleted.
````

## Titles and scope

Use a concise title that names the decision, such as “Represent query results
with `EntitySet`,” rather than the task that produced it. Prefer one coherent
decision per ADR. Use separate, cross-linked records when later work materially
changes or supersedes an earlier choice.

## Dates

The decision date is the best available approximation of when the design was
adopted. Exact precision is not required: a month, year, approximate date, or
`unknown` is preferable to an invented date. The recorded date is when the ADR
itself was authored, which may be much later for reconstructed decisions.

## Status

Use the status that best describes the historical record:

- **Accepted:** The decision was adopted.
- **Deprecated:** The decision is retained for historical context but should no
  longer guide new work, without one specific replacement ADR.
- **Superseded:** A later ADR replaced or materially revised the decision.
- **Proposed:** The decision is under consideration and has not been adopted.

When a decision is superseded, update its status and cross-reference the later
ADR. Do not otherwise rewrite the body to describe the replacement design.

## Historical voice and evidence

Describe the system and constraints as they existed at the time. Distinguish
what an implementation plan proposed from what code and repository history
show was adopted. Summarize the reasoning faithfully, including uncertainty,
and avoid interpreting later implementation as though it had already existed.

Prefer durable traceability when it can be recovered:

- GitHub issues and pull requests.
- Feature branch names.
- Important commits or releases.
- Research artifacts in this book.
- Benchmarks, design discussions, and related ADRs.

Missing metadata does not invalidate an ADR. Clear prose and faithful
reconstruction are more important than uniformity.
