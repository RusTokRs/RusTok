use std::fmt;

use icu_locale_core::LanguageIdentifier;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use uuid::Uuid;

use super::DomainError;

fn validate_identifier(kind: &'static str, value: &str) -> Result<(), DomainError> {
    if value.trim().is_empty() {
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
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(D::Error::custom)
            }
        }
    };
}

string_identifier!(ModuleName, "module");
string_identifier!(EntityName, "entity");
string_identifier!(FieldName, "field");
string_identifier!(LinkName, "link");

/// Canonical language identifier used in entity keys and query scope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LocaleKey(String);

impl LocaleKey {
    pub fn new(value: impl AsRef<str>) -> Result<Self, DomainError> {
        let value = value.as_ref().trim();
        if value.is_empty() {
            return Err(DomainError::EmptyIdentifier { kind: "locale" });
        }

        let locale = value
            .parse::<LanguageIdentifier>()
            .map_err(|_| DomainError::InvalidLocale {
                value: value.to_owned(),
            })?;

        Ok(Self(locale.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for LocaleKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for LocaleKey {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for LocaleKey {
    type Error = DomainError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for LocaleKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
pub struct SchemaIdentity {
    pub module: ModuleName,
    pub entity: EntityName,
}

impl fmt::Display for SchemaIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}::{}", self.module, self.entity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SchemaRef {
    pub module: ModuleName,
    pub entity: EntityName,
    pub version: SchemaVersion,
}

impl SchemaRef {
    pub fn identity(&self) -> SchemaIdentity {
        SchemaIdentity {
            module: self.module.clone(),
            entity: self.entity.clone(),
        }
    }
}

impl fmt::Display for SchemaRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}::{}@{}",
            self.module,
            self.entity,
            self.version.get()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityKey {
    pub tenant_id: Uuid,
    pub schema: SchemaRef,
    pub entity_id: Uuid,
    pub locale: Option<LocaleKey>,
}

/// Typed path through zero or more schema links to one terminal field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FieldPath {
    links: Vec<LinkName>,
    field: FieldName,
}

impl FieldPath {
    pub fn new(field: FieldName) -> Self {
        Self {
            links: Vec::new(),
            field,
        }
    }

    pub fn linked(links: impl IntoIterator<Item = LinkName>, field: FieldName) -> Self {
        Self {
            links: links.into_iter().collect(),
            field,
        }
    }

    pub fn links(&self) -> &[LinkName] {
        &self.links
    }

    pub fn field(&self) -> &FieldName {
        &self.field
    }

    pub fn depth(&self) -> usize {
        self.links.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_and_whitespace_identifiers() {
        assert!(ModuleName::new("").is_err());
        assert!(ModuleName::new("   ").is_err());
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

    #[test]
    fn canonicalizes_locale_keys() {
        assert_eq!(LocaleKey::new("EN-us").unwrap().as_str(), "en-US");
        assert_eq!(
            LocaleKey::new("zh-hant-tw").unwrap().as_str(),
            "zh-Hant-TW"
        );
    }

    #[test]
    fn rejects_invalid_locale_keys() {
        assert!(LocaleKey::new("en_US").is_err());
        assert!(LocaleKey::new("not a locale").is_err());
    }

    #[test]
    fn schema_identity_ignores_version() {
        let reference = SchemaRef {
            module: ModuleName::new("rustok-product").unwrap(),
            entity: EntityName::new("product").unwrap(),
            version: SchemaVersion::new(7),
        };

        assert_eq!(reference.identity().to_string(), "rustok-product::product");
        assert_eq!(reference.to_string(), "rustok-product::product@7");
    }

    #[test]
    fn field_path_separates_links_from_terminal_field() {
        let path = FieldPath::linked(
            [LinkName::new("sales_channel").unwrap()],
            FieldName::new("id").unwrap(),
        );

        assert_eq!(path.links()[0].as_str(), "sales_channel");
        assert_eq!(path.field().as_str(), "id");
        assert_eq!(path.depth(), 1);
    }
}
