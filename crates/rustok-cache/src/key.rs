use sha2::{Digest, Sha256};

const MAX_SAFE_COMPONENT_BYTES: usize = 96;
const MAX_CACHE_KEY_BYTES: usize = 512;

/// Canonical builder for versioned, tenant-aware cache keys.
///
/// Fixed namespace components are validated strictly. Dynamic identity components are
/// kept readable when they are short and safe; otherwise they are replaced by a SHA-256
/// digest so user-controlled input cannot create ambiguous or unbounded Redis keys.
#[derive(Debug, Clone)]
pub struct CacheKeyBuilder {
    fixed_prefix: Vec<String>,
    components: Vec<String>,
}

impl CacheKeyBuilder {
    pub fn new(
        service: impl Into<String>,
        environment: impl Into<String>,
        tenant_or_global: impl Into<String>,
        domain: impl Into<String>,
        schema_version: impl Into<String>,
        resource: impl Into<String>,
    ) -> Result<Self, CacheKeyError> {
        let fixed_prefix = [
            ("service", service.into()),
            ("environment", environment.into()),
            ("tenant_or_global", tenant_or_global.into()),
            ("domain", domain.into()),
            ("schema_version", schema_version.into()),
            ("resource", resource.into()),
        ]
        .into_iter()
        .map(|(name, value)| validate_fixed_component(name, value))
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            fixed_prefix,
            components: Vec::new(),
        })
    }

    /// Add a dynamic identity component.
    ///
    /// Empty identities are rejected. Safe ASCII components remain readable; all other
    /// values are represented as `h-<sha256>`.
    pub fn identity(mut self, value: impl AsRef<[u8]>) -> Result<Self, CacheKeyError> {
        self.components.push(canonical_identity(value.as_ref())?);
        Ok(self)
    }

    /// Add a named identity component while retaining the field name in the key.
    pub fn named_identity(
        mut self,
        name: impl Into<String>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, CacheKeyError> {
        let name = validate_fixed_component("identity_name", name.into())?;
        let value = canonical_identity(value.as_ref())?;
        self.components.push(name);
        self.components.push(value);
        Ok(self)
    }

    /// Always hash a dynamic component, including binary input or canonical query bytes.
    pub fn hashed(mut self, value: impl AsRef<[u8]>) -> Result<Self, CacheKeyError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(CacheKeyError::EmptyIdentity);
        }
        self.components.push(format!("h-{}", sha256_hex(value)));
        Ok(self)
    }

    pub fn build(self) -> String {
        let mut all = self.fixed_prefix.clone();
        all.extend(self.components);
        let key = all.join(":");
        if key.len() <= MAX_CACHE_KEY_BYTES {
            return key;
        }

        // Preserve the human-readable namespace and hash only the overlong identity tail.
        format!(
            "{}:key-h-{}",
            self.fixed_prefix.join(":"),
            sha256_hex(key.as_bytes())
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheKeyError {
    EmptyFixedComponent { name: &'static str },
    InvalidFixedComponent { name: &'static str, value: String },
    FixedComponentTooLong {
        name: &'static str,
        length: usize,
        maximum: usize,
    },
    EmptyIdentity,
}

impl std::fmt::Display for CacheKeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyFixedComponent { name } => {
                write!(formatter, "cache key component `{name}` must not be empty")
            }
            Self::InvalidFixedComponent { name, value } => write!(
                formatter,
                "cache key component `{name}` contains unsupported characters: {value:?}"
            ),
            Self::FixedComponentTooLong {
                name,
                length,
                maximum,
            } => write!(
                formatter,
                "cache key component `{name}` is {length} bytes; maximum is {maximum}"
            ),
            Self::EmptyIdentity => write!(formatter, "cache key identity must not be empty"),
        }
    }
}

impl std::error::Error for CacheKeyError {}

fn validate_fixed_component(
    name: &'static str,
    value: String,
) -> Result<String, CacheKeyError> {
    if value.is_empty() {
        return Err(CacheKeyError::EmptyFixedComponent { name });
    }
    if value.len() > MAX_SAFE_COMPONENT_BYTES {
        return Err(CacheKeyError::FixedComponentTooLong {
            name,
            length: value.len(),
            maximum: MAX_SAFE_COMPONENT_BYTES,
        });
    }
    if !is_safe_component(value.as_bytes()) {
        return Err(CacheKeyError::InvalidFixedComponent { name, value });
    }
    Ok(value)
}

fn canonical_identity(value: &[u8]) -> Result<String, CacheKeyError> {
    if value.is_empty() {
        return Err(CacheKeyError::EmptyIdentity);
    }

    if value.len() <= MAX_SAFE_COMPONENT_BYTES && is_safe_component(value) {
        // Safety check above guarantees ASCII and therefore valid UTF-8.
        return Ok(std::str::from_utf8(value)
            .expect("safe cache key bytes are ASCII")
            .to_string());
    }

    Ok(format!("h-{}", sha256_hex(value)))
}

fn is_safe_component(value: &[u8]) -> bool {
    value.iter().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.')
    })
}

fn sha256_hex(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> CacheKeyBuilder {
        CacheKeyBuilder::new("rustok", "prod", "tenant-a", "catalog", "v2", "product")
            .unwrap()
    }

    #[test]
    fn builds_readable_versioned_tenant_key() {
        let key = base()
            .named_identity("id", "product-42")
            .unwrap()
            .build();

        assert_eq!(
            key,
            "rustok:prod:tenant-a:catalog:v2:product:id:product-42"
        );
    }

    #[test]
    fn unsafe_and_large_identity_is_hashed_deterministically() {
        let raw = "query with spaces and : separators";
        let first = base().identity(raw).unwrap().build();
        let second = base().identity(raw).unwrap().build();

        assert_eq!(first, second);
        assert!(first.contains(":h-"));
        assert!(!first.contains(raw));
    }

    #[test]
    fn tenant_and_version_are_part_of_identity() {
        let first = CacheKeyBuilder::new(
            "rustok", "prod", "tenant-a", "catalog", "v1", "product",
        )
        .unwrap()
        .identity("42")
        .unwrap()
        .build();
        let second = CacheKeyBuilder::new(
            "rustok", "prod", "tenant-b", "catalog", "v2", "product",
        )
        .unwrap()
        .identity("42")
        .unwrap()
        .build();

        assert_ne!(first, second);
    }

    #[test]
    fn overlong_complete_key_preserves_prefix_and_hashes_tail() {
        let builder = base();
        let mut builder = builder;
        for index in 0..20 {
            builder = builder
                .named_identity("filter", format!("value-{index}-{}", "x".repeat(80)))
                .unwrap();
        }

        let key = builder.build();
        assert!(key.starts_with("rustok:prod:tenant-a:catalog:v2:product:key-h-"));
        assert!(key.len() <= MAX_CACHE_KEY_BYTES);
    }

    #[test]
    fn rejects_ambiguous_fixed_namespace_component() {
        assert_eq!(
            CacheKeyBuilder::new("rustok", "prod", "tenant:a", "catalog", "v1", "product")
                .unwrap_err(),
            CacheKeyError::InvalidFixedComponent {
                name: "tenant_or_global",
                value: "tenant:a".to_string(),
            }
        );
    }
}
