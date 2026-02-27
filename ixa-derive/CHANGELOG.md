# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.0-beta.1](https://github.com/CDCgov/ixa/compare/ixa-derive-v2.0.0-beta...ixa-derive-v2.0.0-beta.1) - 2026-02-23

### Other

- Faster precommit ([#764](https://github.com/CDCgov/ixa/pull/764))

## [2.0.0-beta](https://github.com/CDCgov/ixa/compare/ixa-derive-v1.0.0...ixa-derive-v2.0.0-beta) - 2026-02-09

- Added entities implementation and removed people module

# [1.0.0](https://github.com/CDCgov/ixa/compare/ixa-derive-v0.0.3...ixa-derive-v1.0.0) - 2025-11-17

- added rust fmt rules for imports ([#586](https://github.com/CDCgov/ixa/pull/586))

## [0.0.3](https://github.com/CDCgov/ixa/compare/ixa-derive-v0.0.2...ixa-derive-v0.0.3) - 2025-09-22

### Added

- Multi-Properties and Multi-Indexing Refactor ([#518](https://github.com/CDCgov/ixa/pull/518))

## [0.0.2](https://github.com/CDCgov/ixa/compare/ixa-derive-v0.0.1...ixa-derive-v0.0.2) - 2025-06-06

### Other

- Dependencies are now listed in `[workspace.dependencies]` and inherited in packages that depend on them via `my_dependency.workspace = true`.
- Workspace members and examples with their own `Cargo.toml` inherit the values of the fields repository, license, edition, homepage, and authors from the workspace.
- Modified workspace members to opt in to workspace lint exceptions.
