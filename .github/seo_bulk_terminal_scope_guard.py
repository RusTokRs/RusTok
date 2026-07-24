import subprocess

allowed = {
    ".github/seo_bulk_terminal_scope_guard.py",
    ".github/seo_bulk_terminal_test_compat.py",
    ".github/workflows/seo-bulk-terminal-bootstrap-v3.yml",
    ".github/workflows/seo-bulk-terminal-transaction-slice.yml",
    "scripts/seo/apply_bulk_terminal_transaction.py",
    "crates/rustok-seo/src/services/bulk.rs",
    "crates/rustok-seo/src/services/events.rs",
    "crates/rustok-seo/tests/bulk_terminal_transaction.rs",
    "docs/roadmaps/seo-hardening-progress.md",
}
changed = set(
    subprocess.check_output(
        ["git", "diff", "--cached", "--name-only"],
        text=True,
    ).splitlines()
)
unexpected = sorted(changed - allowed)
required = {
    ".github/workflows/seo-bulk-terminal-transaction-slice.yml",
    "scripts/seo/apply_bulk_terminal_transaction.py",
    "crates/rustok-seo/src/services/bulk.rs",
    "crates/rustok-seo/src/services/events.rs",
    "crates/rustok-seo/tests/bulk_terminal_transaction.rs",
    "docs/roadmaps/seo-hardening-progress.md",
}
missing = sorted(required - changed)
if unexpected or missing:
    raise SystemExit(f"invalid SEO slice paths: unexpected={unexpected}, missing={missing}")
