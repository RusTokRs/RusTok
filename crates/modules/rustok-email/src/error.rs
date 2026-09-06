use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("Email sending disabled")]
    Disabled,

    #[error("Invalid email address: {0}")]
    InvalidAddress(String),

    #[error("SMTP configuration error: {0}")]
    SmtpConfig(String),

    #[error("Failed to build email: {0}")]
    Build(String),

    #[error("Failed to send email: {0}")]
    Send(String),

    #[error("Template error: {0}")]
    Template(String),
}

pub type Result<T> = std::result::Result<T, EmailError>;
