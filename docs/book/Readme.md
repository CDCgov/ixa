# The Ixa Book

## Prerequisits

You need mdBook and the `mdbook-callouts` plugins.

```bash
cargo install mdbook mdbook-callouts mdbook-inline-highlighting
```

## Building

To build without opening it:

```bash
mdbook build
```

...or to build and the open the rendered book in your browser:

```bash
mdbook build --open
```

For authoring, use `serve` instead:

```bash
mdbook serve --open
```

> The `serve` command watches the book’s `src` directory for changes, rebuilding the book and refreshing clients for each change; this includes re-creating deleted files still mentioned in `SUMMARY.md`! A websocket connection is used to trigger the client-side refresh.
