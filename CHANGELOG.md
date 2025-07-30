# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0](https://github.com/CDCgov/ixa/compare/ixa-v0.2.1...ixa-v0.3.0) - 2025-07-28

This release makes a breaking change to data plugins. When migrating:

* Look for instances of of `get_data_container` and replace them with `get_data`
* Look for instances of `get_data_container_mut`; if they were purely for initialization of
  the data container, they can be removed or replaced with `get_data`. 
  If a mutable reference is actually needed, replace with `get_data_mut`
* Look for instances of `define_data_plugin`; you may be able
  to move code that was previously outside of the macro into the initializer
  
### Added

- [**breaking**] New data plugin API ([#464](https://github.com/CDCgov/ixa/pull/464))

### Fixed

- Silenced a handful of warnings that are only emitted for the `web_api` feature. ([#469](https://github.com/CDCgov/ixa/pull/469))

### Other

- Made the check for a request of zero sampled people more robust, which potentially avoids a panic deeper in the code path. ([#474](https://github.com/CDCgov/ixa/pull/474))
- enable markdownlint and ignore CHANGELOG.md ([#462](https://github.com/CDCgov/ixa/pull/462))

## [0.2.1](https://github.com/CDCgov/ixa/compare/ixa-v0.2.0...ixa-v0.2.1) - 2025-07-14

### Added

- Added `execution_stats.rs` that implements collecting profiling data. ([#427](https://github.com/CDCgov/ixa/pull/427))

### Fixed

- define_multi_property_index should not references private module ([#450](https://github.com/CDCgov/ixa/pull/450))

### Other

- release-pr should run every 2 weeks ([#446](https://github.com/CDCgov/ixa/pull/446))
- Added a command line flag for a timeline progress bar. ([#426](https://github.com/CDCgov/ixa/pull/426))
- Unorphands numeric.rs, which was accidentally orphaned in #333. ([#443](https://github.com/CDCgov/ixa/pull/443))
- fix define_report macro ( renamed from create_report_trait) ([#447](https://github.com/CDCgov/ixa/pull/447))
- fix performance md ([#445](https://github.com/CDCgov/ixa/pull/445))

## [0.2.0](https://github.com/CDCgov/ixa/compare/ixa-v0.1.2...ixa-v0.2.0) - 2025-07-07

### Added

- Adds `filter_people`, which removes people from a vector. Fixes #435. ([#437](https://github.com/CDCgov/ixa/pull/437))
- Added wasm integration tests with playwright ([#410](https://github.com/CDCgov/ixa/pull/410))
- Added a progress bar feature and modified the logging module to accommodate it. ([#416](https://github.com/CDCgov/ixa/pull/416))
- add sample people ([#422](https://github.com/CDCgov/ixa/pull/422))
- Resources for learning Rust for ixa development ([#412](https://github.com/CDCgov/ixa/pull/412))
- Wasm Compatibility, including a new Wasm logger ([#379](https://github.com/CDCgov/ixa/pull/379))
- Add PluginContext trait for Context ([#385](https://github.com/CDCgov/ixa/pull/385))
- Use a fast pseudorandom number generator ([#380](https://github.com/CDCgov/ixa/pull/380))

### Fixed

- person_property debug trait ([#413](https://github.com/CDCgov/ixa/pull/413))
- Fix paths for mdbook, turn off create-missing ([#395](https://github.com/CDCgov/ixa/pull/395))

### Other

- use PluginContext ([#438](https://github.com/CDCgov/ixa/pull/438))
- Silences a few lints new in the latest Rust release. ([#442](https://github.com/CDCgov/ixa/pull/442))
- edits to book re SIR terminology ([#432](https://github.com/CDCgov/ixa/pull/432))
- Added a section on performance and profiling in the Ixa Book. Fixes #407. ([#425](https://github.com/CDCgov/ixa/pull/425))
- Update context.rs ([#431](https://github.com/CDCgov/ixa/pull/431))
- Made in-source references to input files for examples more robust. Fixes #374. ([#428](https://github.com/CDCgov/ixa/pull/428))
- bump release-plz/action from 0.5.107 to 0.5.108 ([#418](https://github.com/CDCgov/ixa/pull/418))
- add action to Check Conventional Commits ([#415](https://github.com/CDCgov/ixa/pull/415))
- Disable markdown lint ([#420](https://github.com/CDCgov/ixa/pull/420))
- Add release plz config to use conventional commits ([#411](https://github.com/CDCgov/ixa/pull/411))
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
