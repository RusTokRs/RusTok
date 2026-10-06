use crate::checkout_admission::{admit_checkout, ensure_checkout_admitted};
use crate::CartResult;

/// Common admission gate used by checkout lifecycle owners.
///
/// Keeps lifecycle admission decisions in the cart owner boundary before
/// calling persistence transitions.
pub fn require_checkout_admission(begin_possible: bool, lock_exists: bool) -> CartResult<()> {
    let admission = admit_checkout(begin_possible, lock_exists);
    ensure_checkout_admitted(admission)
}
