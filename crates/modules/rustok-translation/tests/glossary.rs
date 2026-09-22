use std::{collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError, TenantLocale};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_tenant::{
    ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyPort,
    TenantLocalePolicyProjection,
};
use rustok_translation::{
    CreateGlossaryInput, CreateJobInput, GlossaryBinding, GlossaryConcept, GlossaryMatchKind,
    GlossaryScope, GlossaryTermPolicy, GlossaryVariant, ReplaceGlossaryTermsInput,
    SetGlossaryActiveInput, TranslationError, TranslationGlossaryService,
    TranslationWorkflowService, UpdateGlossaryInput, migrations,
};
use rustok_translation_targets::{FieldKey, OwnerSlug, ResourceKind, TranslationTargetRegistry};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Default)]
struct FakeTenantLocalePolicies {
    projections: RwLock<BTreeMap<Uuid, TenantLocalePolicyProjection>>,
}

impl FakeTenantLocalePolicies {
    async fn insert(&self, projection: TenantLocalePolicyProjection) {
        self.projections
            .write()
            .await
            .insert(projection.tenant_id, projection);
    }
}

#[async_trait]
impl TenantLocalePolicyPort for FakeTenantLocalePolicies {
    async fn read_locale_policy(
        &self,
        context: PortContext,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        let tenant_id = Uuid::parse_str(&context.tenant_id).unwrap();
        self.projections
            .read()
            .await
            .get(&tenant_id)
            .cloned()
            .ok_or_else(|| {
                PortError::not_found(
                    "translation.test_tenant_policy_missing",
                    "test tenant locale policy is missing",
                )
            })
    }

    async fn replace_locale_policy(
        &self,
        _context: PortContext,
        _request: ReplaceTenantLocalePolicyRequest,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        Err(PortError::unavailable(
            "translation.test_tenant_policy_write_unavailable",
            "test port is read-only",
        ))
    }
}

struct Fixture {
    database: DatabaseConnection,
    glossary: TranslationGlossaryService,
    workflow: TranslationWorkflowService,
}

#[tokio::test]
async fn glossary_crud_is_idempotent_normalized_and_tenant_scoped() {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let fixture = fixture(&[tenant_a, tenant_b]).await;

    let input = glossary_input("  Product terminology  ");
    let created = fixture
        .glossary
        .create_glossary(write_context(tenant_a, "create-glossary"), input.clone())
        .await
        .unwrap();
    assert_eq!(created.name, "Product terminology");
    assert_eq!(created.revision, 1);
    assert!(created.is_active);
    assert!(created.concepts.is_empty());

    let replay = fixture
        .glossary
        .create_glossary(write_context(tenant_a, "create-glossary"), input)
        .await
        .unwrap();
    assert_eq!(replay, created);

    let tenant_a_list = fixture
        .glossary
        .list_glossaries(read_context(tenant_a), 25)
        .await
        .unwrap();
    assert_eq!(tenant_a_list.len(), 1);
    assert_eq!(tenant_a_list[0].id, created.id);
    assert!(
        fixture
            .glossary
            .list_glossaries(read_context(tenant_b), 25)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        fixture
            .glossary
            .read_glossary(read_context(tenant_b), created.id, None)
            .await
            .unwrap_err(),
        TranslationError::GlossaryNotFound
    ));

    let reused_key = fixture
        .glossary
        .create_glossary(
            write_context(tenant_a, "create-glossary"),
            glossary_input("Different glossary"),
        )
        .await
        .unwrap_err();
    assert!(matches!(reused_key, TranslationError::IdempotencyConflict));

    let duplicate_name = fixture
        .glossary
        .create_glossary(
            write_context(tenant_a, "duplicate-name"),
            glossary_input("product TERMINOLOGY"),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate_name,
        TranslationError::GlossaryNameConflict
    ));
}

#[tokio::test]
async fn term_replacement_preserves_revision_snapshots() {
    let tenant_id = Uuid::new_v4();
    let fixture = fixture(&[tenant_id]).await;
    let created = create_glossary(&fixture.glossary, tenant_id, "revisioned").await;

    let revision_two = fixture
        .glossary
        .replace_terms(
            write_context(tenant_id, "terms-v2"),
            ReplaceGlossaryTermsInput {
                glossary_id: created.id,
                expected_revision: 1,
                concepts: vec![
                    concept(
                        "checkout",
                        "Checkout",
                        vec![variant("Kasse", GlossaryTermPolicy::Preferred)],
                    ),
                    concept(
                        "rustok",
                        "RusToK",
                        vec![variant("RusToK", GlossaryTermPolicy::DoNotTranslate)],
                    ),
                ],
            },
        )
        .await
        .unwrap();
    assert_eq!(revision_two.revision, 2);
    assert_eq!(revision_two.concepts.len(), 2);

    let revision_three = fixture
        .glossary
        .replace_terms(
            write_context(tenant_id, "terms-v3"),
            ReplaceGlossaryTermsInput {
                glossary_id: created.id,
                expected_revision: 2,
                concepts: vec![concept(
                    "checkout",
                    "Checkout",
                    vec![
                        variant("Bezahlvorgang", GlossaryTermPolicy::Preferred),
                        variant("Kasse", GlossaryTermPolicy::Allowed),
                    ],
                )],
            },
        )
        .await
        .unwrap();
    assert_eq!(revision_three.revision, 3);
    assert_eq!(revision_three.concepts.len(), 1);

    let old_snapshot = fixture
        .glossary
        .read_glossary(read_context(tenant_id), created.id, Some(2))
        .await
        .unwrap();
    assert_eq!(old_snapshot.revision, 2);
    assert_eq!(old_snapshot.concepts.len(), 2);
    assert_eq!(old_snapshot.concepts[0].variants[0].value, "Kasse");

    let current_snapshot = fixture
        .glossary
        .read_glossary(read_context(tenant_id), created.id, None)
        .await
        .unwrap();
    assert_eq!(current_snapshot, revision_three);

    let persisted_versions: i64 = fixture
        .database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM translation_glossary_terms \
             WHERE tenant_id = ? AND glossary_id = ?",
            [tenant_id.into(), created.id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "count")
        .unwrap();
    assert_eq!(persisted_versions, 4);
}

#[tokio::test]
async fn glossary_rejects_term_conflicts_stale_writes_and_invalid_lifecycle_changes() {
    let tenant_id = Uuid::new_v4();
    let fixture = fixture(&[tenant_id]).await;
    let created = create_glossary(&fixture.glossary, tenant_id, "conflicts").await;

    let duplicate_source = fixture
        .glossary
        .replace_terms(
            write_context(tenant_id, "duplicate-source"),
            ReplaceGlossaryTermsInput {
                glossary_id: created.id,
                expected_revision: 1,
                concepts: vec![
                    concept(
                        "first",
                        "Checkout",
                        vec![variant("Kasse", GlossaryTermPolicy::Preferred)],
                    ),
                    concept(
                        "second",
                        "checkout",
                        vec![variant("Bezahlen", GlossaryTermPolicy::Preferred)],
                    ),
                ],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate_source,
        TranslationError::GlossaryTermConflict(_)
    ));

    let multiple_preferred = fixture
        .glossary
        .replace_terms(
            write_context(tenant_id, "multiple-preferred"),
            ReplaceGlossaryTermsInput {
                glossary_id: created.id,
                expected_revision: 1,
                concepts: vec![concept(
                    "checkout",
                    "Checkout",
                    vec![
                        variant("Kasse", GlossaryTermPolicy::Preferred),
                        variant("Bezahlen", GlossaryTermPolicy::Preferred),
                    ],
                )],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        multiple_preferred,
        TranslationError::GlossaryTermConflict(_)
    ));

    let invalid_do_not_translate = fixture
        .glossary
        .replace_terms(
            write_context(tenant_id, "invalid-dnt"),
            ReplaceGlossaryTermsInput {
                glossary_id: created.id,
                expected_revision: 1,
                concepts: vec![concept(
                    "rustok",
                    "RusToK",
                    vec![variant("Rustok", GlossaryTermPolicy::DoNotTranslate)],
                )],
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        invalid_do_not_translate,
        TranslationError::GlossaryTermConflict(_)
    ));

    let renamed = fixture
        .glossary
        .update_glossary(
            write_context(tenant_id, "rename"),
            UpdateGlossaryInput {
                glossary_id: created.id,
                expected_revision: 1,
                name: "Renamed glossary".to_string(),
                description: "Updated".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.revision, 2);

    assert!(matches!(
        fixture
            .glossary
            .update_glossary(
                write_context(tenant_id, "stale-rename"),
                UpdateGlossaryInput {
                    glossary_id: created.id,
                    expected_revision: 1,
                    name: "Stale".to_string(),
                    description: String::new(),
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryRevisionConflict {
            expected: 1,
            actual: 2
        }
    ));

    let inactive = fixture
        .glossary
        .set_active(
            write_context(tenant_id, "deactivate"),
            SetGlossaryActiveInput {
                glossary_id: created.id,
                expected_revision: 2,
                is_active: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(inactive.revision, 3);
    assert!(!inactive.is_active);
    assert!(matches!(
        fixture
            .glossary
            .set_active(
                write_context(tenant_id, "deactivate-again"),
                SetGlossaryActiveInput {
                    glossary_id: created.id,
                    expected_revision: 3,
                    is_active: false,
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryActiveStateUnchanged
    ));
    assert!(matches!(
        fixture
            .glossary
            .replace_terms(
                write_context(tenant_id, "inactive-write"),
                ReplaceGlossaryTermsInput {
                    glossary_id: created.id,
                    expected_revision: 3,
                    concepts: vec![concept(
                        "checkout",
                        "Checkout",
                        vec![variant("Kasse", GlossaryTermPolicy::Preferred)],
                    )],
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryInactive
    ));
}

#[tokio::test]
async fn jobs_capture_only_current_active_matching_glossary_revisions() {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let fixture = fixture(&[tenant_a, tenant_b]).await;
    let created = create_glossary(&fixture.glossary, tenant_a, "job binding").await;
    let revision_two = fixture
        .glossary
        .replace_terms(
            write_context(tenant_a, "job-terms"),
            ReplaceGlossaryTermsInput {
                glossary_id: created.id,
                expected_revision: 1,
                concepts: vec![concept(
                    "checkout",
                    "Checkout",
                    vec![variant("Kasse", GlossaryTermPolicy::Preferred)],
                )],
            },
        )
        .await
        .unwrap();
    let binding = GlossaryBinding {
        glossary_id: created.id,
        revision: revision_two.revision,
    };

    let job = fixture
        .workflow
        .create_job(
            write_context(tenant_a, "job-with-glossary"),
            CreateJobInput {
                source_locale: locale("en"),
                target_locale: locale("de"),
                glossary: Some(binding.clone()),
            },
        )
        .await
        .unwrap();
    assert_eq!(job.glossary, Some(binding.clone()));

    assert!(matches!(
        fixture
            .workflow
            .create_job(
                write_context(tenant_b, "cross-tenant-glossary"),
                CreateJobInput {
                    source_locale: locale("en"),
                    target_locale: locale("de"),
                    glossary: Some(binding.clone()),
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryNotFound
    ));
    assert!(matches!(
        fixture
            .workflow
            .create_job(
                write_context(tenant_a, "stale-glossary"),
                CreateJobInput {
                    source_locale: locale("en"),
                    target_locale: locale("de"),
                    glossary: Some(GlossaryBinding {
                        glossary_id: created.id,
                        revision: 1,
                    }),
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryRevisionConflict {
            expected: 1,
            actual: 2
        }
    ));
    assert!(matches!(
        fixture
            .workflow
            .create_job(
                write_context(tenant_a, "locale-mismatch"),
                CreateJobInput {
                    source_locale: locale("en"),
                    target_locale: locale("fr"),
                    glossary: Some(binding.clone()),
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryLocaleMismatch
    ));

    let inactive = fixture
        .glossary
        .set_active(
            write_context(tenant_a, "job-deactivate"),
            SetGlossaryActiveInput {
                glossary_id: created.id,
                expected_revision: 2,
                is_active: false,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        fixture
            .workflow
            .create_job(
                write_context(tenant_a, "inactive-glossary"),
                CreateJobInput {
                    source_locale: locale("en"),
                    target_locale: locale("de"),
                    glossary: Some(GlossaryBinding {
                        glossary_id: created.id,
                        revision: inactive.revision,
                    }),
                },
            )
            .await
            .unwrap_err(),
        TranslationError::GlossaryInactive
    ));
}

async fn fixture(tenant_ids: &[Uuid]) -> Fixture {
    let database = Database::connect("sqlite::memory:").await.unwrap();
    database
        .execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .unwrap();
    database
        .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await
        .unwrap();
    let manager = SchemaManager::new(&database);
    SysEventsMigration.up(&manager).await.unwrap();
    for migration in migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    for tenant_id in tenant_ids {
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)",
                [(*tenant_id).into()],
            ))
            .await
            .unwrap();
    }

    let policies = Arc::new(FakeTenantLocalePolicies::default());
    for tenant_id in tenant_ids {
        policies.insert(locale_policy(*tenant_id)).await;
    }
    let glossary = TranslationGlossaryService::new(database.clone(), policies.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone())));
    let workflow = TranslationWorkflowService::new(
        database.clone(),
        Arc::new(TranslationTargetRegistry::default()),
        policies,
        event_bus,
    );
    Fixture {
        database,
        glossary,
        workflow,
    }
}

async fn create_glossary(
    service: &TranslationGlossaryService,
    tenant_id: Uuid,
    key: &str,
) -> rustok_translation::GlossaryRecord {
    service
        .create_glossary(
            write_context(tenant_id, &format!("create-{key}")),
            glossary_input(key),
        )
        .await
        .unwrap()
}

fn glossary_input(name: &str) -> CreateGlossaryInput {
    CreateGlossaryInput {
        name: name.to_string(),
        description: "Tenant terminology policy".to_string(),
        source_locale: locale("en"),
        target_locale: locale("de"),
        scope: GlossaryScope {
            owner_slug: Some(OwnerSlug::new("commerce").unwrap()),
            resource_kind: Some(ResourceKind::new("product").unwrap()),
            field_key: Some(FieldKey::new("title").unwrap()),
        },
    }
}

fn concept(
    concept_key: &str,
    source_term: &str,
    variants: Vec<GlossaryVariant>,
) -> GlossaryConcept {
    GlossaryConcept {
        concept_key: concept_key.to_string(),
        source_term: source_term.to_string(),
        variants,
        match_kind: GlossaryMatchKind::WholeWord,
        case_sensitive: false,
        notes: String::new(),
    }
}

fn variant(value: &str, policy: GlossaryTermPolicy) -> GlossaryVariant {
    GlossaryVariant {
        value: value.to_string(),
        policy,
    }
}

fn locale_policy(tenant_id: Uuid) -> TenantLocalePolicyProjection {
    TenantLocalePolicyProjection {
        tenant_id,
        revision: 1,
        default_locale: locale("en"),
        locales: ["en", "de", "fr"]
            .into_iter()
            .map(|value| TenantLocalePolicyEntry {
                locale: locale(value),
                name: value.to_string(),
                native_name: value.to_string(),
                is_default: value == "en",
                is_enabled: true,
                fallback_locale: (value != "en").then(|| locale("en")),
            })
            .collect(),
    }
}

fn read_context(tenant_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "translation-glossary-read",
    )
    .with_deadline(Duration::from_secs(5))
}

fn write_context(tenant_id: Uuid, idempotency_key: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        format!("translation-glossary-{idempotency_key}"),
    )
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(5))
}

fn locale(value: &str) -> TenantLocale {
    TenantLocale::new(value).unwrap()
}
