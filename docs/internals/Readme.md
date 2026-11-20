# Ixa Internals

This directory contains the developer-facing Ixa Internals mdBook. It includes
historical architectural decision records and research artifacts alongside
maintained developer guides.

## Prerequisites

The repository's `mise.toml` pins mdBook and the required
`mdbook-callouts` and `mdbook-inline-highlighting` plugins. From the repository
root, install the configured tools:

```sh
mise install
```

## Building

Build the Internals book from the repository root:

```sh
mise run docs:internals
```

The rendered book is written to `docs/internals/book/`.

To build and open the rendered book directly:

```sh
mdbook build docs/internals --open
```

For interactive authoring, serve the book with live reload:

```sh
mdbook serve docs/internals --open
```

> The `serve` command watches the book’s `src` directory for changes, rebuilding
> the book and refreshing clients for each change; this includes re-creating
> deleted files still mentioned in `SUMMARY.md`! A websocket connection is used
> to trigger the client-side refresh.
