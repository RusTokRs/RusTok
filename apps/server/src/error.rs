/// Loco error isolation layer.
///
/// All server code should use `crate::error::{Error, Result}` rather than
/// importing `loco_rs::Error` / `loco_rs::Result` directly.  When Loco is
/// upgraded this file is the single place that needs to change.
pub use loco_rs::{Error, Result};

pub fn http_error(error: rustok_web::HttpError) -> Error {
    Error::CustomError(
        error.status,
        loco_rs::controller::ErrorDetail::new(error.code.as_str(), error.message.as_str()),
    )
}
