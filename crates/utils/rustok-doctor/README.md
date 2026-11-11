# rustok-doctor

`rustok-doctor` owns reusable repository hygiene diagnostics. The workspace
`xtask` command and the standalone binary call the same library.

```text
cargo xtask doctor hygiene --crate product
cargo xtask doctor hygiene --changed
cargo xtask doctor hygiene --staged --strict
cargo xtask doctor hotspots --all --top 20
```

The scanner examines Rust source for the rules in the
[hygiene catalog](../../../docs/standards/logic-hygiene-patterns.md). Findings
include both potential impact and confidence. Only high-confidence findings
meet a failure threshold. The scanner selects full files in Git modes and reads
staged content from Git index blobs.

`hotspots` ranks Rust files by line count and labels test files. Generated
and vendored files are counted separately in the exclusions. Use `--json`
for a machine-readable ranking. File size is a review signal.

The library exposes `scan_source`, `Finding`, `Severity`, `Confidence`,
and the CLI entry point `run`. See [the local contract](docs/README.md) for
selection, reporting, and verification details.
