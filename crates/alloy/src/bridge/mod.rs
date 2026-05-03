mod http;
mod utils;

use crate::context::ExecutionPhase;
use email_address::EmailAddress;
use rhai::Engine;

pub use utils::register_utils;

fn validate_email_address(email: &str) -> bool {
    EmailAddress::is_valid(email)
}

pub struct Bridge;

impl Bridge {
    pub fn register_for_phase(engine: &mut Engine, phase: ExecutionPhase) {
        register_utils(engine);

        match phase {
            ExecutionPhase::Before => {
                Self::register_validation_helpers(engine);
            }
            ExecutionPhase::After => {
                Self::register_db_services(engine);
            }
            ExecutionPhase::OnCommit => {
                Self::register_external_services(engine);
            }
            ExecutionPhase::Manual | ExecutionPhase::Scheduled => {
                Self::register_db_services(engine);
                Self::register_external_services(engine);
                Self::register_validation_helpers(engine);
            }
        }
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

    fn register_external_services(engine: &mut Engine) {
        http::register_http(engine);
    }
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
