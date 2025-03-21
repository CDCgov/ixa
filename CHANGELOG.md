# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.2](https://github.com/CDCgov/ixa/compare/ixa-v0.0.1...ixa-v0.0.2) - 2025-03-21

### Added

- Add release-plz action for automated releases ([#277](https://github.com/CDCgov/ixa/pull/277))

### Changed

- changed log-level modules to accept equal sign instead of colon ([#237](https://github.com/CDCgov/ixa/pull/237))

### Fixed

- fix gitignore
- fixed gitignore
- updated .gitignore
- update rusttoolchain in release actions ([#289](https://github.com/CDCgov/ixa/pull/289))
- release action names ([#288](https://github.com/CDCgov/ixa/pull/288))
- Update the derive crate to have correct version ([#285](https://github.com/CDCgov/ixa/pull/285))
- Deterministic `HashMap` and `HashSet` ([#273](https://github.com/CDCgov/ixa/pull/273))
- fixed setup script to not use variables with curl ([#269](https://github.com/CDCgov/ixa/pull/269))
- fixing turbofish syntax errors

### Other

- Updates to the Ixa Book ([#276](https://github.com/CDCgov/ixa/pull/276))
- Revert "Upgraded rand crate dependency to v0.9.0, rand_distr to v0.5.1, and adjusted code throughout to accommodate breaking changes. ([#246](https://github.com/CDCgov/ixa/pull/246))" ([#270](https://github.com/CDCgov/ixa/pull/270))
- Change handling of ctrl-D in debugger to exit program ([#253](https://github.com/CDCgov/ixa/pull/253))
- Fix mdbook build path ([#262](https://github.com/CDCgov/ixa/pull/262))
- Fix deploy path ([#261](https://github.com/CDCgov/ixa/pull/261))
- Fix deploy issues ([#257](https://github.com/CDCgov/ixa/pull/257))
- Build the Ixa Book and deploy to GitHub Pages ([#255](https://github.com/CDCgov/ixa/pull/255))
- added script to setup new ixa projects ([#248](https://github.com/CDCgov/ixa/pull/248))
- Use website for published assets ([#254](https://github.com/CDCgov/ixa/pull/254))
- Ad Ixa book to docs:`docs/book` ([#241](https://github.com/CDCgov/ixa/pull/241))
- Added people commmand to debugger ([#247](https://github.com/CDCgov/ixa/pull/247))
- Return an Option from sample_person ([#245](https://github.com/CDCgov/ixa/pull/245))
- Upgraded rand crate dependency to v0.9.0, rand_distr to v0.5.1, and adjusted code throughout to accommodate breaking changes. ([#246](https://github.com/CDCgov/ixa/pull/246))
- Add build command for runner_test_debug or runner_test_custom_args … ([#232](https://github.com/CDCgov/ixa/pull/232))
- Set log levels for modules #209 ([#231](https://github.com/CDCgov/ixa/pull/231))
- Drop context in report.rs tests ([#229](https://github.com/CDCgov/ixa/pull/229))
- Update network example with transmission tree output ([#148](https://github.com/CDCgov/ixa/pull/148))
- Debugger front end refactor ([#211](https://github.com/CDCgov/ixa/pull/211))
- Adding a dev codespace ([#225](https://github.com/CDCgov/ixa/pull/225))
- Adjusting web path to project root #200 ([#219](https://github.com/CDCgov/ixa/pull/219))
- added create_presicion_report ([#213](https://github.com/CDCgov/ixa/pull/213))
- Disabled the libtest benchmark harness in Cargo.toml. Fixes #217. ([#218](https://github.com/CDCgov/ixa/pull/218))
- Replace `Hash` with `Serialize` for `IndexValue`s. Fixes #189 ([#212](https://github.com/CDCgov/ixa/pull/212))
- Re-export rand, ctor, paste ([#210](https://github.com/CDCgov/ixa/pull/210))
- Remove spurious comma. Fixes #199 ([#206](https://github.com/CDCgov/ixa/pull/206))
- Implement Copy and Clone for QueryAnd ([#203](https://github.com/CDCgov/ixa/pull/203))
- Redirect / ([#202](https://github.com/CDCgov/ixa/pull/202))
- Allow programs to extend web api ([#201](https://github.com/CDCgov/ixa/pull/201))
- Start of the Web console ([#185](https://github.com/CDCgov/ixa/pull/185))
- Fix argument parsing for time-varying-infection ([#197](https://github.com/CDCgov/ixa/pull/197))
- List the available properties ([#195](https://github.com/CDCgov/ixa/pull/195))
- Add an external tabulate API. #192
- Port the main examples to use the runner ([#196](https://github.com/CDCgov/ixa/pull/196))
- Added query_and utility ([#193](https://github.com/CDCgov/ixa/pull/193))
- Benchmarking with Criterion.rs ([#156](https://github.com/CDCgov/ixa/pull/156))
- Refactor out synthesized methodsr ([#188](https://github.com/CDCgov/ixa/pull/188))
- Added logging infrastructure. Fixes #158. ([#174](https://github.com/CDCgov/ixa/pull/174))
- Add an API to get properties for individual people ([#184](https://github.com/CDCgov/ixa/pull/184))
- updated devcontainer files to have a working rust codespaces ([#186](https://github.com/CDCgov/ixa/pull/186))
- Network example ([#134](https://github.com/CDCgov/ixa/pull/134))
- Add support for serving static files ([#183](https://github.com/CDCgov/ixa/pull/183))
- Promoted `people` module to a directory ([#176](https://github.com/CDCgov/ixa/pull/176))
- Move clippy configuration to Cargo.toml ([#178](https://github.com/CDCgov/ixa/pull/178))
- Add a CSRF secret. Fixes #172. ([#175](https://github.com/CDCgov/ixa/pull/175))
- Add a Web API and a generic external API ([#167](https://github.com/CDCgov/ixa/pull/167))
- Add global properties to derived property macro ([#171](https://github.com/CDCgov/ixa/pull/171))
- Restructure debugger to use the clap derived API. ([#166](https://github.com/CDCgov/ixa/pull/166))
- Made both `PersonId` and `PlandId` a tuple-style newtype instead of a single field struct and modified usages accordingly. ([#155](https://github.com/CDCgov/ixa/pull/155))
- Add global command to the debugger ([#142](https://github.com/CDCgov/ixa/pull/142))
- Make clippy allow module name repetitions ([#157](https://github.com/CDCgov/ixa/pull/157))
- Some changes to make the debugger more flexible ([#140](https://github.com/CDCgov/ixa/pull/140))
- Updated basic-infection, births-deaths to use `ixa::people`. Fixes #141. ([#146](https://github.com/CDCgov/ixa/pull/146))
- Bump all outdated dependency versions. ([#152](https://github.com/CDCgov/ixa/pull/152))
- Initial rustyline integration. Fixes #126 ([#133](https://github.com/CDCgov/ixa/pull/133))
- Add the person to edge. Fixes #135 ([#138](https://github.com/CDCgov/ixa/pull/138))
- Fix macro import for define_person_property ([#136](https://github.com/CDCgov/ixa/pull/136))
- Add additional options to runner ([#130](https://github.com/CDCgov/ixa/pull/130))
- Changed `sample_person()` to take a query ([#124](https://github.com/CDCgov/ixa/pull/124))
- Updated runner example ([#129](https://github.com/CDCgov/ixa/pull/129))
- MVP debugger ([#123](https://github.com/CDCgov/ixa/pull/123))
- Add runner module for arg parsing and setup ([#114](https://github.com/CDCgov/ixa/pull/114))
- Properties should use the convention N, NValue ([#121](https://github.com/CDCgov/ixa/pull/121))
- Remove get_person_id() and all its uses ([#119](https://github.com/CDCgov/ixa/pull/119))
- validate on set_global_property_value. Fixes #107
- Re-export from ixa inner modules. Fixes #86 ([#118](https://github.com/CDCgov/ixa/pull/118))
- Update person properties periodic report in example to use real API ([#115](https://github.com/CDCgov/ixa/pull/115))
- You don't need to run CI on non-main pushes ([#113](https://github.com/CDCgov/ixa/pull/113))
- Return error when you try to change a global property. Fixes #71. ([#74](https://github.com/CDCgov/ixa/pull/74))
- Implement add_periodic_report ([#108](https://github.com/CDCgov/ixa/pull/108))
- Minor report docs updates ([#110](https://github.com/CDCgov/ixa/pull/110))
- Add docs for random ([#109](https://github.com/CDCgov/ixa/pull/109))
- Ekr docs global properties ([#106](https://github.com/CDCgov/ixa/pull/106))
- Now that we have API docs, link to them ([#105](https://github.com/CDCgov/ixa/pull/105))
- Add query_people_count(). Fixes #97 ([#101](https://github.com/CDCgov/ixa/pull/101))
- Fix path for IxaError so that you don't need to use ixa ([#100](https://github.com/CDCgov/ixa/pull/100))
- Fix pre-commit
- Fix typo
- Try to auto-trigger
- Target-dir
- Start from a different template
- Add files
- Don't clean
- Fix
- Start of deploy
- Argh
- Fix indent
- Fix yaml
- Fix line ending
- Build docs
- Add documentation for people.rs ([#94](https://github.com/CDCgov/ixa/pull/94))
- Added ParseIntError to IxaError ([#102](https://github.com/CDCgov/ixa/pull/102))
- Ekr validate global properties ([#99](https://github.com/CDCgov/ixa/pull/99))
- More error types ([#96](https://github.com/CDCgov/ixa/pull/96))
- Initial implementation of a network facility. ([#84](https://github.com/CDCgov/ixa/pull/84))
- Load global properties from a configuration file ([#85](https://github.com/CDCgov/ixa/pull/85))
- Report test: remove written files to working dir automatically ([#89](https://github.com/CDCgov/ixa/pull/89))
- Add periodic plan scheduling ([#82](https://github.com/CDCgov/ixa/pull/82))
- Don't automatically overwrite reports ([#83](https://github.com/CDCgov/ixa/pull/83))
- Initialize properties in add_person ([#79](https://github.com/CDCgov/ixa/pull/79))
- Add a query interface as well as indexes. ([#68](https://github.com/CDCgov/ixa/pull/68))
- Fix usages of get_global_property_value in births-deaths ([#78](https://github.com/CDCgov/ixa/pull/78))
- Fix unused warnings in examples ([#75](https://github.com/CDCgov/ixa/pull/75))
- Add an IxaError variant that is IxaError(String). ([#77](https://github.com/CDCgov/ixa/pull/77))
- Births and deaths closes issue #51 ([#65](https://github.com/CDCgov/ixa/pull/65))
- Change |get_global_property()| to return Option<&T::Value>. Fixes #72 ([#73](https://github.com/CDCgov/ixa/pull/73))
- Example of time-varying rates ([#54](https://github.com/CDCgov/ixa/pull/54))
- Add more_plans method to check if more plans to evaluate ([#70](https://github.com/CDCgov/ixa/pull/70))
- Add priorities for plans ([#39](https://github.com/CDCgov/ixa/pull/39))
- Derived properties ([#64](https://github.com/CDCgov/ixa/pull/64))
- Only return person ids for people that exist in the simulation so far ([#62](https://github.com/CDCgov/ixa/pull/62))
- Add global properties and example ([#47](https://github.com/CDCgov/ixa/pull/47))
- Implement display for PersonId ([#58](https://github.com/CDCgov/ixa/pull/58))
- Add initialize_person_property
- Add Rust support for codespaces ([#52](https://github.com/CDCgov/ixa/pull/52))
- Add person properties implementation and example
- Code to implement example #1 (basic infection) ([#40](https://github.com/CDCgov/ixa/pull/40))
- Editorial changes ([#45](https://github.com/CDCgov/ixa/pull/45))
- Create an example for random ([#43](https://github.com/CDCgov/ixa/pull/43))
- Implement report.rs component ([#28](https://github.com/CDCgov/ixa/pull/28))
- Test examples and apply pedantic clippy to tests and examples in CI ([#36](https://github.com/CDCgov/ixa/pull/36))
- Added shutdown to context. Fixes #33
- Switch DataPlugins to use struct type indicator ([#34](https://github.com/CDCgov/ixa/pull/34))
- Added random module ([#18](https://github.com/CDCgov/ixa/pull/18))
- More tests for events ([#31](https://github.com/CDCgov/ixa/pull/31))
- Report Example for Multi-Threading ([#26](https://github.com/CDCgov/ixa/pull/26))
- Update the reports explanation ([#27](https://github.com/CDCgov/ixa/pull/27))
- Added two reports examples ([#23](https://github.com/CDCgov/ixa/pull/23))
- Gce ixa basic example ([#19](https://github.com/CDCgov/ixa/pull/19))
- Fix broken link and docs title ([#14](https://github.com/CDCgov/ixa/pull/14))
- Add Context ([#4](https://github.com/CDCgov/ixa/pull/4))
- Run pedantic clippy in CI ([#7](https://github.com/CDCgov/ixa/pull/7))
- Placeholder gh-pages ([#10](https://github.com/CDCgov/ixa/pull/10))
- Add logos ([#9](https://github.com/CDCgov/ixa/pull/9))
- Preliminary test with pre-commit - resolves #3 ([#5](https://github.com/CDCgov/ixa/pull/5))
