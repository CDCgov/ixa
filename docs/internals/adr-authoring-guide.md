# ADR Authoring Session Guide

Use this guide to author one preassigned historical Architectural Decision
Record (ADR) in an interactive Codex session. This guide, the assignment data
below, and `docs/internals/src/adrs/template.md` contain the durable
instructions needed after the original Internals implementation plan is
archived.

The purpose of a dedicated authoring session is to research, draft, and revise
one ADR with the user. It is not responsible for selecting the next ADR,
renumbering the inventory, integrating navigation, or disposing of source
files. This narrow ownership allows several ADRs to be authored concurrently
without conflicting edits.

## What you are authoring

An ADR is a dated historical snapshot of one significant architectural or
technical decision that was adopted during development. It explains the
problem and constraints that prompted the decision, what was chosen, why it was
chosen, and what trade-offs followed. Its purpose is to preserve reasoning that
would otherwise have to be reconstructed from code, Git history, issues, pull
requests, and institutional memory.

An ADR is not a guide to the current implementation. Describe the system,
constraints, alternatives, and reasoning as they existed when the decision was
made. Later changes do not make the historical record incorrect. If a later
decision replaced the earlier one, record the earlier ADR as superseded and
link it to the later ADR when known rather than rewriting the old decision in
present-day terms.

The authoring task is to reconstruct the decision that was actually adopted,
not merely restate an implementation plan. Plans, notes, research, benchmarks,
code, tests, branches, commits, issues, and pull requests are evidence. Compare
them as needed to distinguish proposals and rejected alternatives from the
implemented outcome.

Each ADR should cover one coherent decision at a useful architectural or
technical level. Several basis files may contribute to one ADR, and closely
related ADRs may cross-reference one another. Follow the assignment's
preselected boundary unless the evidence shows that it should change; in that
case, pause and report the issue rather than silently splitting, merging, or
renumbering ADRs.

Use `docs/internals/src/adrs/template.md` as the canonical detailed authoring
reference. A typical ADR may contain:

- A concise decision-oriented title.
- The best-known decision date and the later date on which the ADR was
  recorded.
- Status, normally accepted, deprecated, or superseded.
- Relevant issue, PR, feature-branch, commit, research, and ADR references.
- A short summary.
- The historical context, problem, and constraints.
- The decision and its rationale.
- Positive consequences, limitations, risks, and trade-offs.
- Alternatives considered and why they were not selected.
- Traceability to the evidence used for the reconstruction.

This is a flexible outline, not a required schema. Historical evidence will
often be incomplete. Omit unavailable or inapplicable metadata and sections
rather than invent facts or add empty boilerplate. State material uncertainty
when it affects the interpretation of the decision.

A finished ADR succeeds when a future developer can understand:

1. What decision was adopted.
2. What circumstances made the decision necessary.
3. Why it was reasonable given the information and constraints at the time.
4. What important consequences and rejected alternatives were known.
5. Where the strongest available historical evidence can be found.

## Required assignment

The user or coordinating session must provide, or point to an inventory row
that provides:

- The exact ADR identifier and tentative output filename.
- The working title.
- The basis files.
- The best-known ordering date and its basis.
- All recovered references and keyed investigation notes.
- When the session uses a separate branch or worktree, the prepared base
  revision and the expected integration method.

Do not infer which ADR is "next" when the assignment is ambiguous. Do not
change an assigned identifier or filename merely because later evidence
suggests a different chronology. Report the evidence and ask the user or
coordinating session to resolve any identity or ordering change.

Before beginning, verify that no other session owns the same ADR or handoff
paths. If the ADR file already exists, treat it as an existing draft to revise
only when the assignment explicitly says to do so. If its handoff file already
exists, stop unless the assignment explicitly says to resume that accepted ADR;
the handoff may already be awaiting integration.

## Not-yet-written ADRs

The following ADRs were deliberately deferred from the initial authoring and
integration batch. Their identifiers remain reserved. This table preserves the
assignment data and historical investigation needed to author them later
without consulting the archived implementation plan.

Do not interpret a row's presence as permission to begin it automatically. The
user must still assign one ADR explicitly. At authoring time, recheck the
branch's current merge status, decision date, PR metadata, and adopted behavior
because the inventory was compiled on 2026-07-29 while the work was unmerged.

| Proposed ADR title or filename | Basis files | Ordering date and basis | Recovered references |
| --- | --- | --- | --- |
| **ADR-0011: Emit events for explicitly initialized properties**<br>`0011-emit-property-initialized-events.md` | `Notes/Notes on Initial Property List.md`<br>`Notes/plan-property-initialized-event.md` | 2026-07-28 — feature-branch commit after rebase onto PR #1009 | Branch `RobertJacobsonCDC_1000_property_initialized_event`; inferred issue #1000; commit `1d2cc61`. The author date is 2026-07-15, but its commit date after rebase is 2026-07-28. No corresponding `main` commit or PR number was found; the branch was unmerged in the local history when investigated. |
| **ADR-0012: Cache creation-event subscriptions**<br>`0012-cache-creation-event-subscriptions.md` | `Notes/Notes on Optimizing PropertyInitializedEvent.md`<br>`Notes/plan-optimize-property-initialized-event.md` | 2026-07-28 — last relevant feature-branch commit | Branch `RobertJacobsonCDC_1000_property_initialized_event`; inferred issue #1000; commits `23ebe26` and `ac75276`. The work was stacked on PR #1009 and remained unmerged in the local history when investigated; no PR number was found. |

## File ownership and parallel-work rules

The authoring session has write ownership of only its matching ADR and handoff
pair:

```text
docs/internals/src/adrs/NNNN-provisional-slug.md
docs/internals/adr-handoffs/NNNN-provisional-slug.md
```

The handoff is a temporary, tracked sidecar outside `src/`, so mdBook does not
publish it. No two sessions append to a common queue file. The presence of one
sidecar per accepted ADR makes `docs/internals/adr-handoffs/` the integration
queue without creating concurrent edits.

The session may inspect any relevant notes, code, tests, documentation, Git
history, branches, commits, and existing ADRs. Unless the user explicitly
expands the assignment, do not edit:

- `docs/internals/src/SUMMARY.md`.
- The ADR landing page or index.
- This guide or its not-yet-written ADR inventory.
- ADR templates or other shared documentation.
- `docs/internals/book.toml` or repository task configuration.
- Any other ADR file.
- Any other ADR handoff file.
- Any basis file under `Notes` or `docs/internals`.
- Anything under `Notes/processed/`.

In particular, do not move or delete basis files. One source may support
several ADRs being authored in parallel. Navigation updates, index changes,
cross-ADR cleanup, and source disposition belong to the later serialized
integration step.

Before editing, inspect the working tree and preserve unrelated user or session
changes. If the assigned ADR file contains overlapping edits that cannot be
attributed safely, stop and ask for direction.

Separate sessions should begin from the same prepared revision containing this
guide, the assigned filenames, ADR template, and mdBook structure. Because
`Notes/` may be locally excluded from Git, a separate worktree must receive the
basis-file contents or accessible paths explicitly; the commit alone may not
provide them.

When working on a separate branch or worktree, do not merge, rebase, or modify
shared files as part of ADR authoring. Do not create a commit unless the user
requests one. Prefer to commit only after acceptance. An accepted-ADR commit
should contain only the assigned ADR and handoff files. Record the branch in
the handoff, then report the resulting commit hash to the user after committing;
a file cannot reliably contain the hash of the commit that contains that same
file. If the user explicitly requests a draft commit before acceptance, commit
only the ADR draft and do not create a premature handoff.

## Authoring procedure

### 1. Load the assignment and guidance

Read:

1. This guide in full.
2. The assigned row under **Not-yet-written ADRs** and any additional evidence
   supplied in the coordinating prompt.
3. The ADR template and authoring guidance at
   `docs/internals/src/adrs/template.md`. If it is missing, report that the
   authoring prerequisites are incomplete.
4. Every assigned basis file.
5. Any existing ADRs identified as predecessors, successors, or closely
   related decisions.

Treat the inventory title, date, filename, and references as well-supported
starting points, not as proof. Preserve useful prior investigation rather than
repeating it without reason.

### 2. Reconstruct and verify the decision

Determine what decision was actually adopted, the problem and constraints at
the time, the alternatives considered, the rationale, and the consequences.
Separate the intended design in an implementation plan from the implementation
that was ultimately merged.

Begin with the recovered references. Verify them using local repository
evidence and, when available and useful, durable GitHub evidence:

- Search for a matching `RobertJacobsonCDC_` feature branch.
- Treat a leading number after that prefix as a likely GitHub issue number,
  subject to confirmation.
- Match feature work to `main` through ancestry when possible, otherwise using
  equivalent patches, commit subjects, dates, and changed files.
- Inspect the matched `main` commit message for a parenthesized PR number.
- Check later history when it may have deprecated, superseded, or materially
  qualified the decision.

Do not perform exhaustive archaeology merely to fill optional metadata. Record
uncertainty honestly and use only the date precision supported by the evidence.
If new evidence would change the ADR's assigned identity, filename, or place in
the chronology, pause and report it before making that shared planning change.

### 3. Draft only the assigned ADR

Create or update the exact assigned file under
`docs/internals/src/adrs/`. Follow the flexible ADR outline in the Internals
authoring guidance. Include the details supported and useful for this decision;
omit inapplicable fields and unsupported boilerplate.

Write in historical voice. Describe the system, constraints, and available
alternatives as they existed when the decision was adopted. Do not rewrite the
past to make it agree silently with the present implementation.

Prefer durable references to GitHub issues, pull requests, and commits when
available. Retain a feature branch name when it is a useful historical clue,
even if the branch may later be deleted. Do not link readers to transient
working notes as though they were permanent documentation; incorporate their
relevant evidence into the ADR.

### 4. Check the draft

Before presenting the draft:

- Confirm that it records one coherent decision.
- Verify the title, identifier, and filename against the assignment.
- Check that the decision date and status match the available evidence.
- Confirm that factual claims can be traced to the source cluster or repository
  history.
- Distinguish adopted behavior from rejected alternatives and later changes.
- Check Markdown formatting and links within the file.
- Run focused, non-mutating documentation checks when available.

Do not add the ADR to `SUMMARY.md` merely to make the full mdBook build include
it. Full navigation and mdBook validation occur during serialized integration.

### 5. Pause for interactive review

Present the completed draft to the user and stop. Summarize:

- The decision reconstructed.
- The strongest historical evidence and traceability recovered.
- Important uncertainties or departures from the inventory.
- The exact ADR file created or revised.

Do not start another ADR, update shared files, move sources, or perform the
integration step. Apply requested changes to this ADR interactively and pause
again after each substantive revision. Continue until the user explicitly
accepts the ADR.

### 6. Prepare the integration handoff

Only after the user explicitly accepts the ADR, create its matching sidecar:

```text
docs/internals/adr-handoffs/NNNN-provisional-slug.md
```

Use this structure:

```markdown
# Integration handoff: ADR-NNNN

- ADR file: `docs/internals/src/adrs/NNNN-provisional-slug.md`
- Suggested summary entry: `[ADR-NNNN: Title](adrs/NNNN-provisional-slug.md)`
- Status: Accepted
- Authoring branch: branch name, same working tree, or not applicable

## Index metadata

- Decision date:
- Title:
- Status:

## Cross-references

- Add:
- Verify:

## Source disposition

- May move after:
- Still needed by:

## Unresolved questions

- None.

## Validation performed

- Checks already completed.
- Checks requiring the integrated mdBook.
```

Keep the handoff compact, but include enough information for an integration
session to proceed without reconstructing the authoring conversation. Use
`None` explicitly where a section was considered and has no entries. If a
source supports another proposed ADR, name that dependency under **Still needed
by** rather than declaring the source ready to move.

Leave the actual shared-file edits and source movements to the designated
integration session. Do not delete the sidecar after creating it. The
integration session removes it only after the ADR has been wired into the book
and integrated validation succeeds. Until then, its presence means that work
remains in the queue.

## Suggested session request

A coordinating prompt may use this form:

> Read `docs/internals/adr-authoring-guide.md` and
> `docs/internals/src/adrs/template.md`. Author only ADR NNNN, using its row
> under **Not-yet-written ADRs** and the supplied basis-file access. Do not edit
> shared navigation, indexes, this guide, templates, or source notes. Present
> the draft and pause for my review before doing anything else.
