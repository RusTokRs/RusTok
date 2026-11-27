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
Target options are mutually exclusive: combining selectors returns exit code 2
instead of silently choosing one. `--strict` and `--fail-on` are also exclusive.

The Git selectors examine full files. `--changed` selects tracked changes
relative to HEAD and untracked files. `--diff REF` selects tracked changes
relative to a commit and `--staged` reads the staged blobs, even if the
working-tree files have since changed. Git selectors require Git.

## Hygiene findings

The hygiene catalog owns the meaning and confidence of rules. The scanner
combines lexical checks with a best-effort syn AST pass and reports candidates
for owner review. It does not resolve Rust types, expand macros, run rustc, or
follow interprocedural data flow. If a file cannot be parsed as a complete Rust
source file, AST-only checks are skipped and lexical checks continue.

The AST pass currently emits heuristic findings for cancellation-unsafe method
calls in select! branch futures (excluding generic read/write names because
those also identify cancellation-safe I/O methods), manual unsafe Send/Sync
impls, panic-capable operations in non-unwinding extern functions, and reversed
self-field lock acquisition order across methods in one impl. The lock-order
check only models direct top-level local guard bindings and explicit
drop(guard); it does not prove lock types or overlapping lifetimes. Test-only
files, build scripts, vendored files, and inline test scopes are excluded from
these production findings.

The column-filtered update-trigger heuristic inspects migration SQL literals.
It cannot establish which migrations are active, whether a BEFORE trigger
changes a watched column, or whether a rollback definition is currently in use.
Review the owner migration chain and database dialect before classifying a match.
The literal-include rule (`P-ZERO-INCLUDE`) reports every direct
`include!("file.rs")` outside test and build contexts, including the contracted
mounted-facade family (`graphql/mutations/safe_cart.rs`,
`graphql/mutations/safe_legacy_helpers.rs`,
`graphql/mutations/safe_checkout.rs`, `graphql/safe_query/source.rs`,
`storefront_checkout_runtime_mounted.rs`,
`services/checkout_compensation_error_safe.rs`, and `rustok-payment`'s
`checkout_execution.rs`). In that family a verify contract pins the include as
the terminal state of a mounted-copy cutover, so the finding is a known false
positive; see the mounted-facade exemption boundary under `P-ZERO-INCLUDE` and
`P-COPY-DRIFT-MOUNT` in the hygiene catalog. The scanner does not yet model
that boundary, and no path-based exemption is hardcoded.
Discarding a success value after `?` does not by itself discard the error;
the ignored-result heuristic excludes that syntactic form.
Files named `test_support.rs` or ending in `_test_support.rs` follow the same
test-file convention as files in `tests/`: production-only rules are skipped.
The `rustok-test-utils` crate is also treated as test-only, as its owner
contract defines it exclusively as test fixtures and helpers.
The scanner does not resolve a file's module declarations or feature gates.
JSON decoding followed directly by `unwrap_or_default()` is reported for review:
the scanner cannot determine whether a compatibility fallback is appropriate.
It also recognizes direct transaction finalization followed by `.await.ok()`;
it cannot establish database-driver recovery behavior.
`filter_map` with `Result::ok` or a nested `.ok()` is a candidate for silently
dropping failed items; partial-result policies remain with the owning component.
These JSON and `filter_map` checks inspect at most 16 source lines from the
start of the call and do not follow values across intermediate bindings.

Selection supports a crate, a path, all modules, the workspace, staged Git
blobs, modified and untracked working-tree files, and files changed relative to
a Git commit. Target selection and source-reading failures exit with code 2.
High-confidence findings meeting the requested threshold exit with code 1.

Text, JSON, CSV, Markdown, GitHub annotations, and advisory patches report the
same selected findings. The console summary lists up to ten examples per rule;
JSON, CSV, Markdown, annotations, and patches include every selected finding.
JSON includes an `evidence` object on each finding. Its `kind` is `lexical`, `ast`,
`rustc`, or `clippy`. Compiler findings also include the lint `code` and the
one-based, character-counted primary `column`. Text, CSV, Markdown, and GitHub
annotations display the source-check kind or lint code.
Git selections examine full files. Patches preserve
the scanned content and append comments with the original line numbers.
Requested report files are written even for an empty result and alongside JSON
output. Report paths must be distinct from each other, scanned files, and the
imported diagnostics file.
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

## Importing Cargo diagnostics

Run a scoped Cargo command in a trusted checkout and capture its JSON stdout
as UTF-8. Import that capture with the same checkout and desired Doctor
selection. For example, from the repository root with `target/` present:

```sh
cargo clippy --locked -p rustok-doctor --message-format=json -- -W let_underscore_lock -W static_mut_refs -W 'clippy::let_underscore_future' -W 'clippy::unused_io_amount' -W 'clippy::await_holding_refcell_ref' -W 'clippy::arc_with_non_send_sync' > target/doctor-cargo.jsonl
cargo run --quiet --locked -p rustok-doctor -- hygiene --crate rustok-doctor --cargo-diagnostics target/doctor-cargo.jsonl --json --fail-on P1
```

The option accepts one complete Cargo JSON stream. Use a fresh capture of
the working tree before running a gate. `--changed` and `--diff REF` select
working-tree files. `--staged` selects index blobs and cannot accept a
working-tree capture.

Doctor imports the [cataloged lint codes](../../../../docs/standards/logic-hygiene-patterns.md#rustc-and-clippy-diagnostics)
from warning or error diagnostics at primary spans. It matches source paths
against the current selection, checks reported lines against current source, and
deduplicates the same lint at the same file, line, and column across build targets.
Normal profile selection determines whether tests, examples, build scripts, or
feature-gated code were compiled. Keep Cargo's stderr for its full error messages.

A stream ending in `build-finished` with `success: false` produces findings from
the available diagnostics, marks the build as failed in text, Markdown, and
`cargo_diagnostics.build_succeeded` in JSON, and exits with code 2. Invalid or
truncated captures and stale spans in selected files exit with code 2 before
writing a report. When Cargo reports success, ordinary severity, confidence,
and exit-code behavior apply. A successful build covers only its selected
configuration. The span match checks location and text, so the caller remains
responsible for capture provenance and freshness.

See the [Cargo JSON message contract](https://doc.rust-lang.org/cargo/reference/external-tools.html)
and [rustc span format](https://doc.rust-lang.org/rustc/json.html#diagnostics).

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
