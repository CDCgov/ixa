# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.2](https://github.com/CDCgov/ixa/compare/ixa-derive-v0.0.1...ixa-derive-v0.0.2) - 2025-06-06

### Other

- Dependencies are now listed in `[workspace.dependencies]` and inherited in packages that depend on them via `my_dependency.workspace = true`.
- Workspace members and examples with their own `Cargo.toml` inherit the values of the fields repository, license, edition, homepage, and authors from the workspace.
- Modified workspace members to opt in to workspace lint exceptions.
