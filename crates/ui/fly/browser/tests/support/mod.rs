//! Shared matching helpers for the bundled Fly browser adapter contract tests.
//!
//! Every test in this directory asserts something about the JavaScript that ships as
//! [`FLY_BROWSER_ADAPTER_JS`]. The asset is maintained as *formatted* JavaScript, so a raw
//! `str::contains` comparison silently couples each assertion to the exact place the formatter
//! broke a line: the same call written
//!
//! ```text
//! setAttribute("role", "alert");
//! ```
//!
//! and
//!
//! ```text
//! ensureStatus(
//!   adapter,
//!   PROBLEM_STATUS_SELECTOR,
//!   "problem",
//!   "alert",
//!   "assertive",
//! );
//! ```
//!
//! are different strings and the same contract. Six assertions had drifted that way (a Prettier
//! pass plus two deliberate refactors — request-scoped abort reasons and
//! `normalizedTransportOptions`), which kept `cargo test -p fly-browser --all-targets` failing.
//! The job only became visible once the CI `--lib` flag that skipped this directory was removed.
//!
//! [`contains`] therefore compares *token sequences* instead of raw text: whitespace is removed
//! from both the asset and the expected fragment. JavaScript ignores whitespace between tokens
//! (outside string and template literals), so a fragment matches wherever the formatter chose to
//! wrap it, and an assertion can no longer break on a formatting-only change.
//!
//! Keep fragments whitespace-free: assert on single-token sequences such as `"a", "b"` rather
//! than on prose.

use fly_browser::FLY_BROWSER_ADAPTER_JS;

/// Remove every whitespace character from `value`.
///
/// `char::is_whitespace` covers the ASCII whitespace JavaScript ignores plus the Unicode
/// separators that would otherwise survive a `\s`-based strip.
pub fn fold(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

/// Whether the shipped adapter contains `marker` as a token sequence, ignoring formatting.
pub fn contains(marker: &str) -> bool {
    fold(FLY_BROWSER_ADAPTER_JS).contains(&fold(marker))
}
