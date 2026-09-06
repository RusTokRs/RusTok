use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid access token")]
    InvalidAccessToken,

    #[error("Invalid reset token")]
    InvalidResetToken,

    #[error("Invalid verification token")]
    InvalidVerificationToken,

    #[error("Invalid invite token")]
    InvalidInviteToken,

    #[error("Token encoding failed")]
    TokenEncodingFailed,

    #[error("Password hashing failed")]
    PasswordHashFailed,

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AuthError>;
