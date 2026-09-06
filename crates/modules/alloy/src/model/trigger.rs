use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScriptTrigger {
    Event {
        entity_type: String,
        event: EventType,
    },
    Cron {
        expression: String,
    },
    Manual,
    Api {
        path: String,
        method: HttpMethod,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    BeforeCreate,
    AfterCreate,
    BeforeUpdate,
    AfterUpdate,
    BeforeDelete,
    AfterDelete,
    OnCommit,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BeforeCreate => "before_create",
            Self::AfterCreate => "after_create",
            Self::BeforeUpdate => "before_update",
            Self::AfterUpdate => "after_update",
            Self::BeforeDelete => "before_delete",
            Self::AfterDelete => "after_delete",
            Self::OnCommit => "on_commit",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.parse().ok()
    }
}

impl FromStr for EventType {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "before_create" => Ok(Self::BeforeCreate),
            "after_create" => Ok(Self::AfterCreate),
            "before_update" => Ok(Self::BeforeUpdate),
            "after_update" => Ok(Self::AfterUpdate),
            "before_delete" => Ok(Self::BeforeDelete),
            "after_delete" => Ok(Self::AfterDelete),
            "on_commit" => Ok(Self::OnCommit),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[default]
    GET,
    POST,
    PUT,
    DELETE,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::DELETE => "DELETE",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.parse().ok()
    }
}

impl FromStr for HttpMethod {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "GET" => Ok(Self::GET),
            "POST" => Ok(Self::POST),
            "PUT" => Ok(Self::PUT),
            "DELETE" => Ok(Self::DELETE),
            _ => Err(()),
        }
    }
}
