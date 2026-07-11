mod http;
mod utils;

use crate::context::ExecutionPhase;
use email_address::EmailAddress;
use rhai::Engine;

pub use http::HttpCapabilityBridge;
pub use utils::register_utils;

fn validate_email_address(email: &str) -> bool {
    EmailAddress::is_valid(email)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseCapabilities {
    pub validation_helpers: bool,
    pub db_services: bool,
    pub external_services: bool,
}

impl PhaseCapabilities {
    pub const fn for_phase(phase: ExecutionPhase) -> Self {
        match phase {
            ExecutionPhase::Before => Self {
                validation_helpers: true,
                db_services: false,
                external_services: false,
            },
            ExecutionPhase::After => Self {
                validation_helpers: false,
                db_services: true,
                external_services: false,
            },
            ExecutionPhase::OnCommit => Self {
                validation_helpers: false,
                db_services: false,
                external_services: true,
            },
            ExecutionPhase::Manual | ExecutionPhase::Scheduled => Self {
                validation_helpers: true,
                db_services: true,
                external_services: true,
            },
        }
    }
}

pub struct Bridge;

impl Bridge {
    pub fn capabilities_for_phase(phase: ExecutionPhase) -> PhaseCapabilities {
        PhaseCapabilities::for_phase(phase)
    }

    pub fn register_for_phase(engine: &mut Engine, phase: ExecutionPhase) {
        register_utils(engine);
        let capabilities = Self::capabilities_for_phase(phase);

        if capabilities.validation_helpers {
            Self::register_validation_helpers(engine);
        }
        if capabilities.db_services {
            Self::register_db_services(engine);
        }
        // External services are exposed only by `HttpCapabilityBridge` while a
        // script runs inside `rustok-sandbox`; phase eligibility alone never
        // registers direct infrastructure access on a reusable Alloy engine.
    }

    fn register_validation_helpers(engine: &mut Engine) {
        engine.register_fn("validate_email", |email: &str| -> bool {
            validate_email_address(email)
        });

        engine.register_fn("validate_required", |value: &str| -> bool {
            !value.trim().is_empty()
        });

        engine.register_fn("validate_min_length", |value: &str, min: i64| -> bool {
            value.len() as i64 >= min
        });
        engine.register_fn("validate_min_length", |value: &str, min: i32| -> bool {
            value.len() as i32 >= min
        });
        engine.register_fn("validate_max_length", |value: &str, max: i64| -> bool {
            value.len() as i64 <= max
        });
        engine.register_fn("validate_max_length", |value: &str, max: i32| -> bool {
            value.len() as i32 <= max
        });
        engine.register_fn("validate_range", |value: i64, min: i64, max: i64| -> bool {
            value >= min && value <= max
        });
        engine.register_fn("validate_range", |value: i32, min: i32, max: i32| -> bool {
            value >= min && value <= max
        });
    }

    fn register_db_services(_engine: &mut Engine) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_email_valid() {
        assert!(validate_email_address("user@example.com"));
        assert!(validate_email_address("user+tag@example.com"));
        assert!(validate_email_address("user.name@sub.example.org"));
        assert!(validate_email_address("user_name@example.co.uk"));
    }

    #[test]
    fn test_phase_capabilities_are_explicit() {
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::Before),
            PhaseCapabilities {
                validation_helpers: true,
                db_services: false,
                external_services: false,
            }
        );
        assert_eq!(
            Bridge::capabilities_for_phase(ExecutionPhase::OnCommit),
            PhaseCapabilities {
                validation_helpers: false,
                db_services: false,
                external_services: true,
            }
        );
    }

    #[test]
    fn test_validate_email_invalid() {
        assert!(!validate_email_address("@."));
        assert!(!validate_email_address("a@b."));
        assert!(!validate_email_address("notanemail"));
        assert!(!validate_email_address("@example.com"));
        assert!(!validate_email_address("user@"));
        assert!(!validate_email_address("user@-example.com"));
        assert!(!validate_email_address("user@example-.com"));
        assert!(!validate_email_address(".user@example.com"));
        assert!(!validate_email_address("user.@example.com"));
        assert!(!validate_email_address("us..er@example.com"));
        assert!(!validate_email_address("user@example..com"));
    }
}
