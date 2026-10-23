use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

const SOURCE_SLUG_MAX_BYTES: usize = 64;
const SEMANTIC_KEY_MAX_BYTES: usize = 96;
const AUDIENCE_CURSOR_MAX_BYTES: usize = 512;
const TARGET_ROUTE_MAX_BYTES: usize = 512;

#[derive(Debug, Error, Clone, Eq, PartialEq)]
pub enum NotificationKeyError {
    #[error("{kind} must not be empty")]
    Empty { kind: &'static str },
    #[error("{kind} exceeds the maximum length of {max_bytes} bytes")]
    TooLong {
        kind: &'static str,
        max_bytes: usize,
    },
    #[error("{kind} must start and end with an ASCII lowercase letter or digit")]
    InvalidBoundary { kind: &'static str },
    #[error("{kind} contains invalid character `{character}`")]
    InvalidCharacter { kind: &'static str, character: char },
    #[error("{kind} contains an empty semantic segment")]
    EmptySegment { kind: &'static str },
    #[error("notification audience cursor contains a control character")]
    CursorControlCharacter,
    #[error("notification target route must be a safe root-relative path")]
    InvalidRoute,
}

macro_rules! semantic_key_type {
    ($name:ident, $kind:literal, $max:expr, $allow_dot:expr) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, NotificationKeyError> {
                let value = value.into();
                validate_semantic_key(value.as_str(), $kind, $max, $allow_dot)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl TryFrom<String> for $name {
            type Error = NotificationKeyError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = NotificationKeyError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

semantic_key_type!(
    NotificationSourceSlug,
    "notification source slug",
    SOURCE_SLUG_MAX_BYTES,
    false
);
semantic_key_type!(
    NotificationTypeKey,
    "notification type key",
    SEMANTIC_KEY_MAX_BYTES,
    true
);
semantic_key_type!(
    NotificationTemplateKey,
    "notification template key",
    SEMANTIC_KEY_MAX_BYTES,
    true
);
semantic_key_type!(
    NotificationTargetKind,
    "notification target kind",
    SEMANTIC_KEY_MAX_BYTES,
    true
);

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct NotificationAudienceCursor(String);

impl NotificationAudienceCursor {
    pub fn new(value: impl Into<String>) -> Result<Self, NotificationKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(NotificationKeyError::Empty {
                kind: "notification audience cursor",
            });
        }
        if value.len() > AUDIENCE_CURSOR_MAX_BYTES {
            return Err(NotificationKeyError::TooLong {
                kind: "notification audience cursor",
                max_bytes: AUDIENCE_CURSOR_MAX_BYTES,
            });
        }
        if value.chars().any(char::is_control) {
            return Err(NotificationKeyError::CursorControlCharacter);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for NotificationAudienceCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct NotificationTargetRoute(String);

impl NotificationTargetRoute {
    pub fn new(value: impl Into<String>) -> Result<Self, NotificationKeyError> {
        let value = value.into();
        if value.is_empty() || value.len() > TARGET_ROUTE_MAX_BYTES || value.contains('#') {
            return Err(NotificationKeyError::InvalidRoute);
        }

        let mut parts = value.split('?');
        let path = parts.next().unwrap_or_default();
        let query = parts.next();
        if parts.next().is_some() || !safe_route_path(path) || !query.is_none_or(safe_route_query) {
            return Err(NotificationKeyError::InvalidRoute);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for NotificationTargetRoute {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

fn safe_route_path(path: &str) -> bool {
    !path.is_empty()
        && path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains("//")
        && !path.contains("://")
        && !path.contains('%')
        && !path.contains('\\')
        && !path.chars().any(char::is_whitespace)
        && path
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/-_.~".contains(character))
        && path
            .split('/')
            .all(|segment| segment != "." && segment != "..")
}

fn safe_route_query(query: &str) -> bool {
    !query.is_empty()
        && query.split('&').all(|pair| {
            let Some((key, value)) = pair.split_once('=') else {
                return false;
            };
            !key.is_empty()
                && !value.is_empty()
                && key.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '-' | '_')
                })
                && value.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
        })
}

fn validate_semantic_key(
    value: &str,
    kind: &'static str,
    max_bytes: usize,
    allow_dot: bool,
) -> Result<(), NotificationKeyError> {
    if value.is_empty() {
        return Err(NotificationKeyError::Empty { kind });
    }
    if value.len() > max_bytes {
        return Err(NotificationKeyError::TooLong { kind, max_bytes });
    }

    let first = value
        .chars()
        .next()
        .expect("non-empty notification key has a first character");
    let last = value
        .chars()
        .last()
        .expect("non-empty notification key has a last character");
    if !is_key_alphanumeric(first) || !is_key_alphanumeric(last) {
        return Err(NotificationKeyError::InvalidBoundary { kind });
    }

    for character in value.chars() {
        let allowed = is_key_alphanumeric(character)
            || character == '-'
            || character == '_'
            || (allow_dot && character == '.');
        if !allowed {
            return Err(NotificationKeyError::InvalidCharacter { kind, character });
        }
    }
    if allow_dot && value.split('.').any(str::is_empty) {
        return Err(NotificationKeyError::EmptySegment { kind });
    }
    Ok(())
}

fn is_key_alphanumeric(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit()
}
