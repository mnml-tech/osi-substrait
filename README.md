# osi-substrait

Library for the **OSI core metadata** shape: load documents, validate them, resolve [`SemanticQuery`](src/query/spec.rs) against a model, build a [`LogicalPlan`](src/plan/mod.rs), and emit **SQL** or (with a feature flag) **Substrait**.

- **Spec:** [Open Semantic Interchange](https://github.com/open-semantic-interchange/OSI) — this crate mirrors `osi-schema.json` as Rust types.
- **No storage, no HTTP** — pure in-memory API for use from CLIs, servers, or tests.

## Features

| Feature | Effect |
|---------|--------|
| *(default)* | SQL emission via `emit::sql` |
| `substrait` | Substrait emission via `emit::substrait` (extra dependency) |

## Test

```sh
cargo test
cargo test --features substrait
```

Fixtures live under [`tests/fixtures/`](tests/fixtures).

The `tpcds_example_parses_and_validates` test is **ignored by default**; it needs the [OSI](https://github.com/open-semantic-interchange/OSI) repo cloned **next to** this repository (`../OSI/examples/…`). Run `cargo test -- --include-ignored` after cloning OSI.

## Publishing to crates.io

1. Add `repository = "https://github.com/<you>/osi-substrait"` (and optional `documentation`) under `[package]` in `Cargo.toml`.
2. `cargo publish --dry-run`
3. `cargo publish`
