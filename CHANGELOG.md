# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0](https://github.com/CDCgov/ixa/compare/ixa-v0.1.2...ixa-v0.2.0) - 2025-06-12

### Added

- Add release-plz configuration file
- Add PluginContext trait for Context ([#385](https://github.com/CDCgov/ixa/pull/385))
- Use a fast pseudorandom number generator ([#380](https://github.com/CDCgov/ixa/pull/380))

### Other

- update release-plz config
- updated release-plz config
- Update dependabot.yaml ([#382](https://github.com/CDCgov/ixa/pull/382))
- Remove duplicate `For instance` from docs ([#383](https://github.com/CDCgov/ixa/pull/383))

## [0.1.2](https://github.com/CDCgov/ixa/compare/ixa-v0.1.1...ixa-v0.1.2) - 2025-06-06

### Added

- Added the `ixa-fips` crate as a member of the workspace.
- Add prelude ([#333](https://github.com/CDCgov/ixa/pull/333))
- Add disallowed methods to linter - hashmap, hashset ([#335](https://github.com/CDCgov/ixa/pull/335))
- Support multi-property indexes ([#309](https://github.com/CDCgov/ixa/pull/309))
- Improve display of Option in reports ([#370](https://github.com/CDCgov/ixa/pull/370))

### Fixed

- subscribe_to_event for derived properties should work even before adding people ([#347](https://github.com/CDCgov/ixa/pull/347))

### Other

- Make debugger and web API options with feature flags
- Narrative-style documentation for the Random module ([#361](https://github.com/CDCgov/ixa/pull/361))
- Add dependabot.yaml. Closes #355. ([#356](https://github.com/CDCgov/ixa/pull/356))
- Added a more helpful error message when a duplicate global property causes a panic. ([#373](https://github.com/CDCgov/ixa/pull/373))
- Added the `'static` constraint to `PersonProperty` and changed `PersonProperty + 'static` to `PersonProperty` everywhere. ([#372](https://github.com/CDCgov/ixa/pull/372))
- Dependencies are now listed in `[workspace.dependencies]` and inherited in packages that depend on them via `my_dependency.workspace = true`.
- Workspace members and examples with their own `Cargo.toml` inherit the values of the fields repository, license, edition, homepage, and authors from the workspace.
- Use `FromRepr` instead of `transmute` for `u8` to `USState` conversion.
- Added cache to the github build ([#348](https://github.com/CDCgov/ixa/pull/348))
- Use glob pattern for workspace members ([#345](https://github.com/CDCgov/ixa/pull/345))
- Modified workspace members to opt in to workspace lint exceptions.
- Vendor `almost_eq`, `convergence`, and the `assert_almost_eq!` macro from statrs@0.18.0 (prec.rs), which are implemented on top of the small `approx` crate.
- New benchmark action ([#326](https://github.com/CDCgov/ixa/pull/326))
- Add bench=false to benchmarks package ([#327](https://github.com/CDCgov/ixa/pull/327))
- Move integration tests to unpublished sub crate ([#320](https://github.com/CDCgov/ixa/pull/320))

## [0.1.1](https://github.com/CDCgov/ixa/compare/ixa-v0.1.0...ixa-v0.1.1) - 2025-04-30

### Added

- Breakpoint, "next", and "halt" implementation. Fixes [#163](https://github.com/CDCgov/ixa/pull/163). ([#249](https://github.com/CDCgov/ixa/pull/249))
- Markdown lint to pre-commit ([#298](https://github.com/CDCgov/ixa/pull/298))

### Fixed

- Fix double borrow during property registration ([#312](https://github.com/CDCgov/ixa/pull/312))

### Other

- Restored workspace, correct versions to Cargo.toml ([#321](https://github.com/CDCgov/ixa/pull/321))
- Fixed broken `include` in the Ixa book. Fixes [#302](https://github.com/CDCgov/ixa/pull/302). ([#306](https://github.com/CDCgov/ixa/pull/306))
- Updated setup script to use released ixa in cargo ([#300](https://github.com/CDCgov/ixa/pull/300))
- Fixed extra semicolon ([#313](https://github.com/CDCgov/ixa/pull/313))
- Fixed book summary chapters ([#319](https://github.com/CDCgov/ixa/pull/319))
- Integrated Benchmarks Into CI ([#215](https://github.com/CDCgov/ixa/pull/215))
- Added contributor docs ([#295](https://github.com/CDCgov/ixa/pull/295))

## [0.1.0](https://github.com/CDCgov/ixa/compare/ixa-v0.0.1...ixa-v0.1.0) - 2025-03-21

### Changed

- This is the first pre-production release of ixa 0.1.0, see documentation at [https://ixa.rs/book](https://ixa.rs/book)
