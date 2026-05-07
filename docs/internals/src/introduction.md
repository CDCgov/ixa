# About the Internals Book

The Ixa Internals book preserves developer-facing knowledge about the
architecture of Ixa, investigations that informed its development, and current
project practices. It complements API documentation and user-facing guides by
explaining why important technical choices were made and how maintainers work
with the repository.

The book contains three kinds of documents with different expectations.

## Architectural Decision Records

Architectural Decision Records (ADRs) are dated historical snapshots of
significant architectural or technical decisions. They capture the context,
choice, rationale, alternatives, and consequences as they were understood at
the time.

An ADR is not a guide to the current implementation. A later change may
supersede an ADR without making the earlier record incorrect. Status metadata
and cross-references connect related decisions while preserving the original
reasoning.

## Research Results and Artifacts

Research artifacts record investigations, experiments, benchmarks, and design
comparisons. They preserve useful findings even when the work did not lead to
an implementation, feature branch, issue, or pull request.

Like ADRs, research artifacts are historical snapshots. They should identify
the question investigated and enough context, method, results, and limitations
to interpret their findings.

## Developer Guides

Developer guides explain how to work on Ixa. Unlike ADRs and research
artifacts, they describe current project practices and should be updated when
commands, tools, APIs, or workflows change.

If a developer guide disagrees with the current repository, treat that as
documentation that needs correction rather than as historical context.
