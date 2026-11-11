# RusToK Doctor contract

`rustok-doctor` owns the native hygiene scanner and its finding model. It
provides a reusable library and a standalone binary. `xtask` delegates its
`doctor` command to the same library.

## Selecting files

Run commands from the repository root. With no target option, Doctor inspects
`crates/modules`. When invoked from a subdirectory, it inspects that directory.
Select a single Rust file or directory with `--path`, one
or more comma-separated crate names with `--crate`, all modules with
`--all-modules`, or `crates/`, `apps/`, and `xtask/` with `--all`.
Unknown and ambiguous crate names return exit code 2.

The Git selectors examine full files. `--changed` selects tracked changes
relative to HEAD and untracked files. `--diff REF` selects tracked changes
relative to a commit and `--staged` reads the staged blobs, even if the
working-tree files have since changed. Git selectors require Git.

## Hygiene findings

The hygiene catalog owns the meaning and confidence of rules. The scanner
examines lexical source and reports candidates for owner review. It does not
resolve Rust types or execute database and application behavior.

Selection supports a crate, a path, all modules, the workspace, staged Git
blobs, modified and untracked working-tree files, and files changed relative to
a Git commit. Target selection and source-reading failures exit with code 2.
High-confidence findings meeting the requested threshold exit with code 1.

Text, JSON, CSV, Markdown, GitHub annotations, and advisory patches report the
same selected findings. Git selections examine full files. Patches preserve
the scanned content and append comments with the original line numbers.
Requested report files are written even for an empty result and alongside JSON
output. Report paths must be distinct from each other and scanned files.
GitHub annotations and JSON use separate invocations. Unknown or ambiguous
crate selectors return an operational error.

Use `--severity P0|P1|P2` to filter findings, `--ignore-rule` for a
comma-separated list of pattern IDs, and `--suggest-fixes` for guidance.
`--strict` fails on high-confidence findings at any severity and `--fail-on`
accepts the same severity thresholds. The potential impact and confidence
fields have separate meanings.

```text
cargo xtask doctor hygiene --crate product --json
cargo xtask doctor hygiene --changed --markdown-report hygiene.md
cargo xtask doctor hygiene --staged --patch hygiene.patch
git apply --cached --check hygiene.patch
```

## Large files

`doctor hotspots` ranks the selected Rust files by physical line count.
It accepts the same target selection options as hygiene, plus `--top N` and
`--json`. The default limit is 20. Test files are labeled in the result.
Files under `vendor`, `vendored`, `vendored-*`, or `vendored_*` directories
are counted as vendored exclusions. Comment lines among the first 20 containing
`@generated`, `do not edit`, or `automatically generated` mark a file as
generated. JSON reports include the selected and ranked file counts, exclusion
counts, and each returned file's path, line count, and source or test label.
The ranking is advisory and exits with 0 on successful inspection.

## Verification

```text
cargo test -p rustok-doctor
cargo xtask doctor hygiene --path crates/modules/rustok-product/src
cargo xtask doctor hygiene --staged --strict
cargo xtask doctor hotspots --all --top 20 --json
```
