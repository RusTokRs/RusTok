use chrono::{DateTime, Datelike, Utc};
use object_store::path::Path;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectZone {
    Objects,
    Staging,
}

impl ObjectZone {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Objects => "objects",
            Self::Staging => "staging",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectScope {
    Tenant(Uuid),
    Namespace {
        tenant_id: Uuid,
        owner_id: Uuid,
        instance_id: Uuid,
    },
    Platform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectKey(Path);

impl ObjectKey {
    /// Build `namespace/zone/scope/YYYY/MM/DD/shard/id.extension`.
    pub fn chronological(
        namespace: &str,
        zone: ObjectZone,
        scope: ObjectScope,
        created_at: DateTime<Utc>,
        object_id: Uuid,
        extension: &str,
    ) -> Result<Self, KeyError> {
        let namespace = validated_segment("namespace", namespace, 64)?;
        let extension = validated_extension(extension)?;
        let scope = scope_segment(scope)?;
        let shard = format!("{:02x}", object_id.as_bytes()[15]);
        let raw = format!(
            "{}/{}/{}/{:04}/{:02}/{:02}/{}/{}.{}",
            namespace,
            zone.as_str(),
            scope,
            created_at.year(),
            created_at.month(),
            created_at.day(),
            shard,
            object_id,
            extension
        );
        Ok(Self(Path::from(raw)))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path(self) -> Path {
        self.0
    }
}

impl std::fmt::Display for ObjectKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestObjectKey(Path);

impl DigestObjectKey {
    /// Build `namespace/objects/scope/sha256/aa/bb/full_digest`.
    pub fn sha256(namespace: &str, scope: ObjectScope, digest_hex: &str) -> Result<Self, KeyError> {
        let namespace = validated_segment("namespace", namespace, 64)?;
        if digest_hex.len() != 64
            || !digest_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(KeyError::InvalidSha256Digest);
        }
        let raw = format!(
            "{}/objects/{}/sha256/{}/{}/{}",
            namespace,
            scope_segment(scope)?,
            &digest_hex[..2],
            &digest_hex[2..4],
            digest_hex
        );
        Ok(Self(Path::from(raw)))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path(self) -> Path {
        self.0
    }
}

impl std::fmt::Display for DigestObjectKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

fn scope_segment(scope: ObjectScope) -> Result<String, KeyError> {
    match scope {
        ObjectScope::Tenant(tenant_id) => Ok(format!("tenants/{tenant_id}")),
        ObjectScope::Namespace {
            tenant_id,
            owner_id,
            instance_id,
        } => {
            if tenant_id.is_nil() || owner_id.is_nil() || instance_id.is_nil() {
                return Err(KeyError::InvalidNamespaceIdentity);
            }
            Ok(format!(
                "tenants/{tenant_id}/owners/{owner_id}/instances/{instance_id}"
            ))
        }
        ObjectScope::Platform => Ok("platform".to_string()),
    }
}

fn validated_segment<'a>(
    name: &'static str,
    value: &'a str,
    max: usize,
) -> Result<&'a str, KeyError> {
    if value.is_empty()
        || value.len() > max
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(KeyError::InvalidSegment {
            name,
            value: value.to_string(),
        });
    }
    Ok(value)
}

fn validated_extension(value: &str) -> Result<&str, KeyError> {
    if value.is_empty()
        || value.len() > 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(KeyError::InvalidExtension(value.to_string()));
    }
    Ok(value)
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KeyError {
    #[error("Namespace tenant, owner, and instance identities must be non-nil")]
    InvalidNamespaceIdentity,
    #[error("invalid {name} segment `{value}`")]
    InvalidSegment { name: &'static str, value: String },
    #[error("invalid object extension `{0}`")]
    InvalidExtension(String),
    #[error("invalid lowercase SHA-256 digest")]
    InvalidSha256Digest,
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn chronological_key_is_date_partitioned_and_sharded() {
        let tenant_id = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap();
        let object_id = Uuid::parse_str("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbb2f").unwrap();
        let created_at = Utc.with_ymd_and_hms(2026, 7, 22, 13, 45, 0).unwrap();
        let key = ObjectKey::chronological(
            "media",
            ObjectZone::Objects,
            ObjectScope::Tenant(tenant_id),
            created_at,
            object_id,
            "webp",
        )
        .unwrap();
        assert_eq!(
            key.to_string(),
            "media/objects/tenants/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa/2026/07/22/2f/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbb2f.webp"
        );
    }

    #[test]
    fn namespace_keys_isolate_owners_and_instances_for_the_same_object() {
        let tenant_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let instance_id = Uuid::new_v4();
        let object_id = Uuid::new_v4();
        let created_at = Utc::now();
        let key = |owner_id, instance_id| {
            ObjectKey::chronological(
                "module-artifact-data",
                ObjectZone::Objects,
                ObjectScope::Namespace {
                    tenant_id,
                    owner_id,
                    instance_id,
                },
                created_at,
                object_id,
                "bin",
            )
            .unwrap()
        };
        let original = key(owner_id, instance_id);
        assert_ne!(original, key(Uuid::new_v4(), instance_id));
        assert_ne!(original, key(owner_id, Uuid::new_v4()));
        assert!(original.to_string().contains(&format!(
            "tenants/{tenant_id}/owners/{owner_id}/instances/{instance_id}/"
        )));
        for scope in [
            ObjectScope::Namespace {
                tenant_id: Uuid::nil(),
                owner_id,
                instance_id,
            },
            ObjectScope::Namespace {
                tenant_id,
                owner_id: Uuid::nil(),
                instance_id,
            },
            ObjectScope::Namespace {
                tenant_id,
                owner_id,
                instance_id: Uuid::nil(),
            },
        ] {
            assert_eq!(
                ObjectKey::chronological(
                    "module-artifact-data",
                    ObjectZone::Objects,
                    scope,
                    created_at,
                    object_id,
                    "bin"
                ),
                Err(KeyError::InvalidNamespaceIdentity)
            );
            assert_eq!(
                DigestObjectKey::sha256("module-artifact-data", scope, &"a".repeat(64)),
                Err(KeyError::InvalidNamespaceIdentity)
            );
        }
    }

    #[test]
    fn digest_key_is_stable_and_date_free() {
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let key =
            DigestObjectKey::sha256("module-artifact", ObjectScope::Platform, digest).unwrap();
        assert_eq!(
            key.to_string(),
            format!("module-artifact/objects/platform/sha256/01/23/{digest}")
        );
    }

    #[test]
    fn key_segments_reject_uncontrolled_paths() {
        let now = Utc::now();
        assert!(
            ObjectKey::chronological(
                "../media",
                ObjectZone::Objects,
                ObjectScope::Platform,
                now,
                Uuid::new_v4(),
                "jpg"
            )
            .is_err()
        );
        assert!(
            ObjectKey::chronological(
                "media",
                ObjectZone::Objects,
                ObjectScope::Platform,
                now,
                Uuid::new_v4(),
                "JPG"
            )
            .is_err()
        );
    }
}
