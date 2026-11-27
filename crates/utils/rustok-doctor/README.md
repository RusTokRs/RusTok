# rustok-doctor

`rustok-doctor` owns reusable repository hygiene diagnostics. The workspace
`xtask` command and the standalone binary call the same library.

```text
cargo xtask doctor hygiene --crate product
cargo xtask doctor hygiene --changed
cargo xtask doctor hygiene --staged --strict
cargo xtask doctor hotspots --all --top 20
```

The scanner combines lexical checks with a best-effort syn AST pass
for the rules in the [hygiene catalog](../../../docs/standards/logic-hygiene-patterns.md).
AST findings are heuristic: the scanner does not resolve Rust types, expand
macros, or follow interprocedural data flow. Findings include both potential
impact and confidence, along with their evidence source. Only high-confidence
findings meet a failure threshold.
The scanner selects full files in Git modes and reads staged content from Git
index blobs.

Pass `--cargo-diagnostics FILE` to include selected rustc and Clippy findings
from a Cargo JSON capture of the working tree. Doctor reads the capture
without running a build. See the [capture workflow](docs/README.md#importing-cargo-diagnostics).

`hotspots` ranks Rust files by line count and labels test files. Generated
and vendored files are counted separately in the exclusions. Use `--json`
for a machine-readable ranking. File size is a review signal.

The library exposes `scan_source`, `Finding`, `Severity`, `Confidence`, `Evidence`,
and the CLI entry point `run`. See [the local contract](docs/README.md) for
selection, reporting, and verification details.

## Known Limitations / Pending Implementation

Imported findings cover the selected files and the build's active targets,
features, and enabled lints. The input provides the lines of emitted diagnostics
only. Run a fresh capture from the exact working tree when gating changes.
Owner-aware data flow, dependency policy, API compatibility, and evidence from
runtime or mutation tests remain separate verification steps.
