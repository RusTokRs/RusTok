use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

fn validate_identifier(kind: &'static str, value: &str) -> Result<(), DomainError> {
    if value.is_empty() {
        return Err(DomainError::EmptyIdentifier { kind });
    }

    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'))
    {
        Ok(())
    } else {
        Err(DomainError::InvalidIdentifier {
            kind,
            value: value.to_owned(),
        })
    }
}

macro_rules! string_identifier {
    ($name:ident, $kind:literal) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
                let value = value.into();
                validate_identifier($kind, &value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = DomainError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = DomainError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
    };
}

string_identifier!(ModuleName, "module");
string_identifier!(EntityName, "entity");
string_identifier!(FieldName, "field");
string_identifier!(LinkName, "link");
string_identifier!(LocaleKey, "locale");

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct SchemaVersion(u32);

impl SchemaVersion {
    pub const INITIAL: Self = Self(1);

    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SchemaRef {
    pub module: ModuleName,
    pub entity: EntityName,
    pub version: SchemaVersion,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityKey {
    pub tenant_id: Uuid,
    pub schema: SchemaRef,
    pub entity_id: Uuid,
    pub locale: Option<LocaleKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FieldPath(Vec<FieldName>);

impl FieldPath {
    pub fn new(parts: impl IntoIterator<Item = FieldName>) -> Result<Self, DomainError> {
        let parts = parts.into_iter().collect::<Vec<_>>();
        if parts.is_empty() {
            return Err(DomainError::EmptyIdentifier { kind: "field path" });
        }
        Ok(Self(parts))
    }

    pub fn parts(&self) -> &[FieldName] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_and_whitespace_identifiers() {
        assert!(ModuleName::new("").is_err());
        assert!(FieldName::new("display name").is_err());
    }

    #[test]
    fn accepts_names_used_by_schema_and_links() {
        assert_eq!(
            ModuleName::new("rustok-product").unwrap().as_str(),
            "rustok-product"
        );
        assert_eq!(
            FieldName::new("updated_at").unwrap().as_str(),
            "updated_at"
        );
    }
}
