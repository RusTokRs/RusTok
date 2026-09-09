use rustok_index::{
    EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion,
    infrastructure::postgres::{
        IndexDriftDigestFindingRequest, IndexDriftFindingScope, IndexDriftFindingSeverity,
    },
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const FINDING_KEY_CONTRACT: &[u8] = b"index_drift_finding_key_v1";
const CHECK_NAME: &str = "source_index_digest_mismatch";
const DIGEST_BYTES: usize = 64;

#[test]
fn locale_bearing_key_bytes_remain_compatible_and_no_locale_is_distinct() {
    let tenant_id = Uuid::from_u128(1);
    let entity_id = Uuid::from_u128(2);
    let schema = SchemaRef {
        module: ModuleName::new("rustok-product").unwrap(),
        entity: EntityName::new("product").unwrap(),
        version: SchemaVersion::new(2),
    };
    let locale = LocaleKey::new("en-US").unwrap();
    let locale_scope = IndexDriftFindingScope::Entity {
        schema: schema.clone(),
        entity_id,
        locale: locale.clone(),
    };
    let locale_request = request(tenant_id, locale_scope);
    let legacy_key = legacy_locale_key(tenant_id, CHECK_NAME, &schema, entity_id, &locale);
    assert_eq!(locale_request.finding_key(), legacy_key.as_str());

    let no_locale_request = request(
        tenant_id,
        IndexDriftFindingScope::EntityWithoutLocale { schema, entity_id },
    );
    assert_ne!(
        no_locale_request.finding_key(),
        locale_request.finding_key()
    );
    assert_eq!(no_locale_request.finding_key().len(), DIGEST_BYTES);
    assert!(
        no_locale_request
            .finding_key()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    );
}

fn request(tenant_id: Uuid, scope: IndexDriftFindingScope) -> IndexDriftDigestFindingRequest {
    IndexDriftDigestFindingRequest::new(
        tenant_id,
        CHECK_NAME,
        IndexDriftFindingSeverity::Error,
        scope,
        "a".repeat(DIGEST_BYTES),
        "b".repeat(DIGEST_BYTES),
    )
    .expect("fixture request must satisfy the bounded finding contract")
}

fn legacy_locale_key(
    tenant_id: Uuid,
    check_name: &str,
    schema: &SchemaRef,
    entity_id: Uuid,
    locale: &LocaleKey,
) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, FINDING_KEY_CONTRACT);
    hash_component(&mut hasher, tenant_id.as_bytes());
    hash_component(&mut hasher, check_name.as_bytes());
    hash_component(&mut hasher, b"entity");
    hash_component(&mut hasher, schema.module.as_str().as_bytes());
    hash_component(&mut hasher, schema.entity.as_str().as_bytes());
    hash_component(&mut hasher, &schema.version.get().to_be_bytes());
    hash_component(&mut hasher, entity_id.as_bytes());
    hash_component(&mut hasher, locale.as_str().as_bytes());
    hex::encode(hasher.finalize())
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}
