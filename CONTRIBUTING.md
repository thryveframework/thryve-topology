# Contributing to Thryve Topology

Thanks for your interest in improving the Thryve topology viewer.

## Getting set up

```bash
git clone https://github.com/thryveframework/thryve-topology.git
cd thryve-topology
cargo build
```

The viewer depends on none of [Thryve](https://github.com/thryveframework/thryve)'s crates: it reads the export with its own description of the file, in [`thryve_graph/src/format.rs`](thryve_graph/src/format.rs). If Thryve's export changes, update `format.rs` to match, and add an export from the new Thryve to `thryve_graph/tests/fixtures`, so the tests keep reading both the old files and the new ones.

## Before opening a PR

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three run in CI and must pass.

## Reporting bugs / requesting features

Open a [GitHub issue](https://github.com/thryveframework/thryve-topology/issues) with:

- What you expected vs. what happened.
- Your OS and the app version (or commit hash if built from source).
- If it's a rendering/parsing issue with a `topology_export.ron` file, attach a minimal reproducing file if you can (redact any sensitive file paths first).

## License

By contributing, you agree that your contributions will be licensed under the project's [AGPL-3.0-or-later license](LICENSE).
