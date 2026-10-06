use crate::CartResult;

/// Owner-side decision used before checkout lifecycle transitions.
///
/// This intentionally avoids exposing transport-specific status strings to
/// checkout callers. Lifecycle owners can extend the decision without forcing
/// commerce adapters to duplicate transition rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutAdmission {
    Allowed,
    AlreadyLocked,
    Rejected { reason: &'static str },
}

impl CheckoutAdmission {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn reject(reason: &'static str) -> Self {
        Self::Rejected { reason }
    }
}

/// Maps lifecycle intent into an owner decision.
///
/// The actual persistence transition remains in CartService. This helper is a
/// single place for future lock/state admission rules.
pub fn admit_checkout(begin_possible: bool, lock_exists: bool) -> CheckoutAdmission {
    if lock_exists {
        return CheckoutAdmission::AlreadyLocked;
    }

    if begin_possible {
        CheckoutAdmission::Allowed
    } else {
        CheckoutAdmission::reject("checkout transition is not allowed")
    }
}

pub fn ensure_checkout_admitted(admission: CheckoutAdmission) -> CartResult<()> {
    match admission {
        CheckoutAdmission::Allowed => Ok(()),
        CheckoutAdmission::AlreadyLocked => Err(crate::CartError::InvalidTransition {
            from: "checkout_locked".into(),
            to: "checkout_locked".into(),
        }),
        CheckoutAdmission::Rejected { reason } => {
            Err(crate::CartError::Validation(reason.into()))
        }
    }
}
