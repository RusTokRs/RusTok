# rustok-doctor

`rustok-doctor` owns reusable repository hygiene diagnostics. The workspace
`xtask` command and the standalone binary call the same library.

```text
cargo xtask doctor hygiene --crate product
cargo xtask doctor hygiene --changed
cargo xtask doctor hygiene --staged --strict
```

The scanner examines Rust source for the rules in the
[hygiene catalog](../../../docs/standards/logic-hygiene-patterns.md). Findings
include both potential impact and confidence. Only high-confidence findings
meet a failure threshold. The scanner selects full files in Git modes and reads
staged content from Git index blobs.

The library exposes `scan_source`, `Finding`, `Severity`, `Confidence`,
and the CLI entry point `run`. See [the local contract](docs/README.md) for
selection, reporting, and verification details.
