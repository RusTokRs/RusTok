from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"{label}: start marker not found")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise SystemExit(f"{label}: end marker not found")
    return text[:start_index] + replacement + text[end_index:]


meta_path = Path("crates/rustok-seo/src/services/meta.rs")
meta = meta_path.read_text()
meta = replace_once(
    meta,
    "    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait,",
    "    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,\n    TransactionTrait,",
    "meta database transaction import",
)

publish_revision_start = "    pub async fn publish_revision("
publish_revision_end = "    pub async fn rollback_revision("
publish_revision_replacement = '''    pub async fn publish_revision(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        note: Option<String>,
    ) -> SeoResult<SeoRevisionRecord> {
        let txn = self.db.begin().await?;
        let Some(explicit) = self
            .load_explicit_meta_in_tx(&txn, tenant.id, target_kind.clone(), target_id)
            .await?
        else {
            return Err(SeoError::NotFound);
        };
        let latest_revision = seo_revision::Entity::find()
            .filter(seo_revision::Column::TenantId.eq(tenant.id))
            .filter(seo_revision::Column::TargetKind.eq(target_kind.as_str()))
            .filter(seo_revision::Column::TargetId.eq(target_id))
            .order_by_desc(seo_revision::Column::Revision)
            .one(&txn)
            .await?;
        let next_revision = latest_revision.map(|item| item.revision + 1).unwrap_or(1);
        let now = chrono::Utc::now().fixed_offset();

        let revision = seo_revision::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            target_kind: Set(target_kind.as_str().to_string()),
            target_id: Set(target_id),
            revision: Set(next_revision),
            note: Set(trimmed_option(note)),
            payload: Set(snapshot_payload(explicit)),
            created_at: Set(now),
        }
        .insert(&txn)
        .await?;

        let record = SeoRevisionRecord {
            id: revision.id,
            target_kind,
            target_id,
            revision: revision.revision,
            note: revision.note,
            created_at: revision.created_at.into(),
        };

        self.publish_seo_revision_published_event_in_tx(
            &txn,
            tenant.id,
            record.target_kind.as_str(),
            record.target_id,
            record.revision,
        )
        .await?;
        txn.commit().await?;

        Ok(record)
    }

'''
meta = replace_between(
    meta,
    publish_revision_start,
    publish_revision_end,
    publish_revision_replacement,
    "revision creation transaction",
)

load_explicit_marker = "    pub(super) async fn load_explicit_meta("
load_explicit_helper = '''    async fn load_explicit_meta_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
    ) -> SeoResult<Option<LoadedMeta>> {
        let Some(meta) = seo_meta::Entity::find()
            .filter(seo_meta::Column::TenantId.eq(tenant_id))
            .filter(seo_meta::Column::TargetType.eq(target_kind.as_str()))
            .filter(seo_meta::Column::TargetId.eq(target_id))
            .one(txn)
            .await?
        else {
            return Ok(None);
        };
        let translations = meta_translation::Entity::find()
            .filter(meta_translation::Column::MetaId.eq(meta.id))
            .order_by_asc(meta_translation::Column::Locale)
            .all(txn)
            .await?;
        Ok(Some(LoadedMeta { meta, translations }))
    }

'''
meta = replace_once(
    meta,
    load_explicit_marker,
    load_explicit_helper + load_explicit_marker,
    "revision snapshot transaction helper",
)
meta_path.write_text(meta)


events_path = Path("crates/rustok-seo/src/services/events.rs")
events = events_path.read_text()
helper_name = "publish_meta_reindex_in_tx"
helper_count = events.count(helper_name)
if helper_count != 2:
    raise SystemExit(f"entity reindex helper rename: expected two matches, found {helper_count}")
events = events.replace(helper_name, "publish_entity_reindex_in_tx")
events = replace_once(
    events,
    "failed to enqueue SEO metadata reindex event transactionally: {error}",
    "failed to enqueue SEO entity reindex event transactionally: {error}",
    "entity reindex error context",
)

revision_event_start = "    pub(super) async fn publish_seo_revision_published_event("
revision_event_end = "    pub(super) async fn publish_seo_revision_rolled_back_event("
revision_event_replacement = '''    pub(super) async fn publish_seo_revision_published_event_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        revision: i32,
    ) -> SeoResult<()> {
        let event_type = "seo.revision.published";
        let idempotency_key = self.build_event_key(
            event_type,
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                revision.to_string(),
            ],
        );
        let existing = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_event_delivery::Column::IdempotencyKey.eq(idempotency_key.as_str()))
            .one(txn)
            .await?;
        if existing.is_some() {
            return Ok(());
        }

        let event = DomainEvent::SeoRevisionPublished {
            target_kind: target_kind.to_string(),
            target_id,
            revision,
            idempotency_key: idempotency_key.clone(),
        };
        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, None, event.clone())
            .await
            .map_err(|error| {
                SeoError::Database(DbErr::Custom(format!(
                    "failed to enqueue SEO revision published event transactionally: {error}"
                )))
            })?;
        let now = Utc::now().fixed_offset();

        seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            event_type: Set(event_type.to_string()),
            idempotency_key: Set(idempotency_key.clone()),
            source_kind: Set(Some(target_kind.to_string())),
            source_id: Set(Some(target_id)),
            status: Set(DELIVERY_STATUS_SENT.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            dispatched_at: Set(Some(now)),
        }
        .insert(txn)
        .await?;

        for trigger in index_reindex_triggers_for_event(&event) {
            self.publish_entity_reindex_in_tx(
                txn,
                tenant_id,
                event_type,
                idempotency_key.as_str(),
                &trigger,
                now,
            )
            .await?;
        }

        Ok(())
    }

'''
events = replace_between(
    events,
    revision_event_start,
    revision_event_end,
    revision_event_replacement,
    "revision published transactional event",
)
events_path.write_text(events)


test_path = Path("crates/rustok-seo/tests/meta_transaction.rs")
test = test_path.read_text()
test = replace_once(
    test,
    "    self as seo_meta, meta_translation, seo_event_delivery, seo_index_cursor,\n    seo_index_delivery,",
    "    self as seo_meta, meta_translation, seo_event_delivery, seo_index_cursor,\n    seo_index_delivery, seo_revision,",
    "revision test entity import",
)
test = replace_once(
    test,
    "use sea_orm::{\n    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,\n    PaginatorTrait, Statement,\n};",
    "use sea_orm::ActiveValue::Set;\nuse sea_orm::{\n    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,\n    EntityTrait, PaginatorTrait, Statement,\n};",
    "revision test ORM imports",
)
test = replace_once(
    test,
    "failed to enqueue SEO metadata reindex event transactionally",
    "failed to enqueue SEO entity reindex event transactionally",
    "metadata test generic reindex error",
)

page_slug_marker = "fn page_slug() -> SeoTargetSlug {"
revision_test = '''#[tokio::test]
async fn revision_creation_rolls_back_when_reindex_event_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "SEO revision tenant".to_string(),
        slug: "seo-revision".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let target_id = Uuid::new_v4();
    let meta_id = Uuid::new_v4();
    seo_meta::ActiveModel {
        id: Set(meta_id),
        tenant_id: Set(tenant.id),
        target_type: Set(page_slug().into_string()),
        target_id: Set(target_id),
        no_index: Set(false),
        no_follow: Set(false),
        canonical_url: Set(None),
        structured_data: Set(None),
    }
    .insert(&db)
    .await
    .expect("explicit metadata should be seeded");
    meta_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        meta_id: Set(meta_id),
        locale: Set("en".to_string()),
        title: Set(Some("Revision title".to_string())),
        description: Set(Some("Revision description".to_string())),
        keywords: Set(None),
        og_title: Set(None),
        og_description: Set(None),
        og_image: Set(None),
    }
    .insert(&db)
    .await
    .expect("explicit metadata translation should be seeded");

    let service = SeoService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailOnSecondTransport::new())),
        Arc::new(SeoTargetRegistry::default()),
    );
    let error = service
        .publish_revision(
            &tenant,
            page_slug(),
            target_id,
            Some("Atomic revision".to_string()),
        )
        .await
        .expect_err("reindex failure must abort revision creation");
    assert!(
        error
            .to_string()
            .contains("failed to enqueue SEO entity reindex event transactionally")
    );

    assert_eq!(
        seo_meta::Entity::find()
            .count(&db)
            .await
            .expect("metadata count should load"),
        1
    );
    assert_eq!(
        meta_translation::Entity::find()
            .count(&db)
            .await
            .expect("translation count should load"),
        1
    );
    assert_eq!(
        seo_revision::Entity::find()
            .count(&db)
            .await
            .expect("revision count should load"),
        0
    );
    assert_eq!(
        seo_event_delivery::Entity::find()
            .count(&db)
            .await
            .expect("event delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_delivery::Entity::find()
            .count(&db)
            .await
            .expect("index delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_cursor::Entity::find()
            .count(&db)
            .await
            .expect("index cursor count should load"),
        0
    );
}

'''
test = replace_once(
    test,
    page_slug_marker,
    revision_test + page_slug_marker,
    "revision creation rollback test",
)

revision_table_marker = '''        "CREATE TABLE seo_event_deliveries (
'''
revision_table = '''        "CREATE TABLE seo_revisions (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL,
            target_id TEXT NOT NULL,
            revision INTEGER NOT NULL,
            note TEXT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        "CREATE UNIQUE INDEX idx_seo_revisions_target_revision
            ON seo_revisions (tenant_id, target_kind, target_id, revision)",
'''
test = replace_once(
    test,
    revision_table_marker,
    revision_table + revision_table_marker,
    "revision test table",
)
test_path.write_text(test)


roadmap_path = Path("docs/roadmaps/seo-hardening-progress.md")
roadmap = roadmap_path.read_text()
roadmap = replace_once(
    roadmap,
    "- [ ] Persist revision creation and its event transactionally.",
    "- [x] Persist revision creation and its event transactionally. (revision creation PR)",
    "revision creation roadmap item",
)
roadmap = replace_once(
    roadmap,
    "- [ ] Add rollback coverage for metadata and revision transactions. Metadata rollback is covered; revision creation and rollback remain open. (#2056)",
    "- [ ] Add rollback coverage for metadata and revision transactions. Metadata and revision creation rollback are covered; revision rollback remains open. (#2056, revision creation PR)",
    "revision rollback coverage roadmap item",
)
roadmap_path.write_text(roadmap)
