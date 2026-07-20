use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use super::trigger::ScriptTrigger;
use super::workspace::AlloyWorkspace;

pub type ScriptId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStatus {
    #[default]
    Draft,
    Active,
    Paused,
    Disabled,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub id: ScriptId,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub workspace: AlloyWorkspace,
    pub trigger: ScriptTrigger,
    pub status: ScriptStatus,
    pub version: u32,
    pub run_as_system: bool,
    pub permissions: Vec<String>,
    pub author_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub error_count: u32,
    pub last_error_at: Option<DateTime<Utc>>,
}

/// Immutable source evidence for one durable Alloy script revision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScriptSourceRevision {
    pub script_id: ScriptId,
    pub tenant_id: Uuid,
    pub revision: u32,
    pub parent_revision: Option<u32>,
    pub source_digest: String,
    pub workspace: AlloyWorkspace,
    pub author_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Script {
    pub fn new(name: impl Into<String>, workspace: AlloyWorkspace, trigger: ScriptTrigger) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            tenant_id: Uuid::nil(),
            name: name.into(),
            description: None,
            workspace,
            trigger,
            status: ScriptStatus::Draft,
            version: 1,
            run_as_system: false,
            permissions: Vec::new(),
            author_id: None,
            created_at: now,
            updated_at: now,
            error_count: 0,
            last_error_at: None,
        }
    }

    pub fn is_executable(&self) -> bool {
        self.status == ScriptStatus::Active
    }

    pub fn register_error(&mut self) -> bool {
        self.error_count += 1;
        self.last_error_at = Some(Utc::now());
        self.error_count >= 3
    }

    pub fn reset_errors(&mut self) {
        self.error_count = 0;
        self.last_error_at = None;
    }

    pub fn activate(&mut self) {
        self.status = ScriptStatus::Active;
        self.reset_errors();
        self.updated_at = Utc::now();
    }

    pub fn disable(&mut self) {
        self.status = ScriptStatus::Disabled;
        self.updated_at = Utc::now();
    }

    pub fn archive(&mut self) {
        self.status = ScriptStatus::Archived;
        self.updated_at = Utc::now();
    }
}

impl ScriptStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Disabled => "disabled",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.parse().ok()
    }
}

impl FromStr for ScriptStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "disabled" => Ok(Self::Disabled),
            "archived" => Ok(Self::Archived),
            _ => Err(()),
        }
    }
}
