# RusToK Doctor contract

`rustok-doctor` owns the native hygiene scanner and its finding model. It
provides a reusable library and a standalone binary. `xtask` delegates its
`doctor` command to the same library.

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

```text
cargo test -p rustok-doctor
cargo xtask doctor hygiene --path crates/modules/rustok-product/src
cargo xtask doctor hygiene --staged --strict
```
