use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandDescriptor {
    pub namespace: String,
    pub name: String,
    pub summary: String,
    pub supports_dry_run: bool,
}

impl CommandDescriptor {
    pub fn new(
        namespace: impl Into<String>,
        name: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
            summary: summary.into(),
            supports_dry_run: false,
        }
    }

    pub fn with_dry_run(mut self) -> Self {
        self.supports_dry_run = true;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRequest {
    pub namespace: String,
    pub name: String,
    pub args: serde_json::Value,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandOutcome {
    pub exit_code: i32,
    pub message: String,
    pub data: serde_json::Value,
}

impl CommandOutcome {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            exit_code: 0,
            message: message.into(),
            data: serde_json::Value::Null,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }
}

#[derive(Debug, Error)]
pub enum CliCoreError {
    #[error("unknown command: {namespace} {name}")]
    UnknownCommand { namespace: String, name: String },
    #[error("invalid command input: {message}")]
    InvalidInput { message: String },
    #[error("command failed: {message}")]
    CommandFailed { message: String },
}

pub type CliCoreResult<T> = Result<T, CliCoreError>;

pub trait CommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor>;
}
