use std::collections::HashSet;

use async_trait::async_trait;
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, SetTaxonomyCategoryPresentationInput, TaxonomyCategoryMediaId,
    TaxonomyCategoryMediaReferenceValidator, TaxonomyError, TaxonomyModule, TaxonomyResult,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::DatabaseConnection;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TaxonomyService) {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    let service = TaxonomyService::new(db.clone());
    (db, service)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    name: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: name.to_string(),
                slug: None,
                canonical_key: Some(format!("{}-{}", name.to_ascii_lowercase(), Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("term should be created")
}

struct FakeMediaValidator {
    tenant_id: Uuid,
    public_images: HashSet<Uuid>,
}

impl FakeMediaValidator {
    fn new(tenant_id: Uuid, public_images: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            tenant_id,
            public_images: public_images.into_iter().collect(),
        }
    }
}

#[async_trait]
impl TaxonomyCategoryMediaReferenceValidator for FakeMediaValidator {
    async fn validate_public_image_reference(
        &self,
        tenant_id: Uuid,
        media_id: TaxonomyCategoryMediaId,
    ) -> TaxonomyResult<()> {
        let media_id = media_id.into_uuid();
        if tenant_id != self.tenant_id || !self.public_images.contains(&media_id) {
            return Err(TaxonomyError::validation(
                "Media reference is not an active public image in this tenant",
            ));
        }
        Ok(())
    }
}

#[tokio::test]
async fn category_without_presentation_reads_as_empty_revision_zero() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = create_term(&service, tenant_id, TaxonomyTermKind::Category, "Engineering").await;

    let presentation = service
        .get_category_presentation(tenant_id, admin(), category_id)
        .await
        .expect("empty canonical presentation should load");

    assert_eq!(presentation.term_id, category_id);
    assert_eq!(presentation.icon_key, None);
    assert_eq!(presentation.color, None);
    assert_eq!(presentation.image_media_id, None);
    assert_eq!(presentation.cover_media_id, None);
    assert_eq!(presentation.revision, 0);
}

#[tokio::test]
async fn canonical_presentation_normalizes_tokens_and_validates_media() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = create_term(&service, tenant_id, TaxonomyTermKind::Category, "Support").await;
    let image_id = Uuid::new_v4();
    let cover_id = Uuid::new_v4();
    let validator = FakeMediaValidator::new(tenant_id, [image_id, cover_id]);

    let presentation = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: Some("  Message-Square  ".to_string()),
                color: Some(" #F0A ".to_string()),
                image_media_id: Some(image_id.into()),
                cover_media_id: Some(cover_id.into()),
                expected_revision: Some(0),
            },
            Some(&validator),
        )
        .await
        .expect("valid canonical presentation should persist");

    assert_eq!(presentation.icon_key.as_deref(), Some("message-square"));
    assert_eq!(presentation.color.as_deref(), Some("#ff00aa"));
    assert_eq!(presentation.image_media_id.map(Into::<Uuid>::into), Some(image_id));
    assert_eq!(presentation.cover_media_id.map(Into::<Uuid>::into), Some(cover_id));
    assert_eq!(presentation.revision, 1);

    let read_back = service
        .get_category_presentation(tenant_id, admin(), category_id)
        .await
        .expect("canonical presentation should read back");
    assert_eq!(read_back, presentation);
}

#[tokio::test]
async fn presentation_noop_keeps_revision_and_changed_write_uses_cas() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = create_term(&service, tenant_id, TaxonomyTermKind::Category, "General").await;

    let first = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: Some("messages-square".to_string()),
                color: Some("#ABCDEF".to_string()),
                image_media_id: None,
                cover_media_id: None,
                expected_revision: Some(0),
            },
            None,
        )
        .await
        .expect("first presentation should persist");
    assert_eq!(first.revision, 1);
    assert_eq!(first.color.as_deref(), Some("#abcdef"));

    let noop = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: Some(" messages-square ".to_string()),
                color: Some("#abcdef".to_string()),
                image_media_id: None,
                cover_media_id: None,
                expected_revision: Some(1),
            },
            None,
        )
        .await
        .expect("normalized no-op should succeed");
    assert_eq!(noop.revision, 1);

    let second = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: Some("messages-square".to_string()),
                color: Some("#11223344".to_string()),
                image_media_id: None,
                cover_media_id: None,
                expected_revision: Some(1),
            },
            None,
        )
        .await
        .expect("changed presentation should advance revision");
    assert_eq!(second.revision, 2);
    assert_eq!(second.color.as_deref(), Some("#11223344"));

    let stale = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: None,
                color: None,
                image_media_id: None,
                cover_media_id: None,
                expected_revision: Some(1),
            },
            None,
        )
        .await
        .expect_err("stale presentation writer must fail closed");
    assert!(matches!(stale, TaxonomyError::Conflict(_)));
}

#[tokio::test]
async fn presentation_rejects_tags_and_missing_media_capability() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let tag_id = create_term(&service, tenant_id, TaxonomyTermKind::Tag, "Rust").await;

    let tag_error = service
        .set_category_presentation(
            tenant_id,
            admin(),
            tag_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: Some("tag".to_string()),
                color: None,
                image_media_id: None,
                cover_media_id: None,
                expected_revision: Some(0),
            },
            None,
        )
        .await
        .expect_err("Tag must not acquire Category presentation");
    assert!(matches!(tag_error, TaxonomyError::Validation(_)));

    let category_id = create_term(&service, tenant_id, TaxonomyTermKind::Category, "Media").await;
    let media_error = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: None,
                color: None,
                image_media_id: Some(Uuid::new_v4().into()),
                cover_media_id: None,
                expected_revision: Some(0),
            },
            None,
        )
        .await
        .expect_err("Media reference without owner validation must fail closed");
    assert!(matches!(media_error, TaxonomyError::Validation(_)));
}

#[tokio::test]
async fn media_validator_rejects_non_public_or_cross_tenant_reference() {
    let (_db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let category_id = create_term(&service, tenant_id, TaxonomyTermKind::Category, "Images").await;
    let allowed_id = Uuid::new_v4();
    let rejected_id = Uuid::new_v4();
    let validator = FakeMediaValidator::new(tenant_id, [allowed_id]);

    let error = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: None,
                color: None,
                image_media_id: Some(rejected_id.into()),
                cover_media_id: None,
                expected_revision: Some(0),
            },
            Some(&validator),
        )
        .await
        .expect_err("non-public Media reference must fail owner validation");
    assert!(matches!(error, TaxonomyError::Validation(_)));

    let foreign_validator = FakeMediaValidator::new(Uuid::new_v4(), [allowed_id]);
    let error = service
        .set_category_presentation(
            tenant_id,
            admin(),
            category_id,
            SetTaxonomyCategoryPresentationInput {
                icon_key: None,
                color: None,
                image_media_id: Some(allowed_id.into()),
                cover_media_id: None,
                expected_revision: Some(0),
            },
            Some(&foreign_validator),
        )
        .await
        .expect_err("cross-tenant Media reference must fail owner validation");
    assert!(matches!(error, TaxonomyError::Validation(_)));
}
