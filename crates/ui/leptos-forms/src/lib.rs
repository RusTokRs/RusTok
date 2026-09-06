mod error;
mod field;
mod form;
mod validator;

pub use error::FormError;
pub use field::Field;
pub use form::FormContext;
pub use validator::Validator;

/// Hook для создания form context
pub fn use_form() -> FormContext {
    FormContext::new()
}
