use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZodIssue {
    pub path: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZodError {
    pub issues: Vec<ZodIssue>,
}

impl ZodError {
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn from_api(issues: Vec<ZodIssue>) -> Self {
        Self { issues }
    }
}
