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
meta = replace_between(
    meta,
    "    pub(super) async fn upsert_meta_with_transition(",
    "    pub async fn publish_revision(",
    '''    pub(super) async fn upsert_meta_with_transition(
        &self,
        tenant: &TenantContext,
        input: SeoMetaInput,
        transition_ref: Option<String>,
    ) -> SeoResult<SeoMetaRecord> {
        let response_locale = self.prepare_meta_transition(tenant, &input).await?;
        let target_kind = input.target_kind.clone();
        let target_id = input.target_id;
        let txn = self.db.begin().await?;

        self.persist_meta_transition_in_tx(
            &txn,
            tenant,
            input,
            response_locale.as_str(),
            transition_ref.as_deref(),
        )
        .await?;
        txn.commit().await?;

        self.seo_meta(
            tenant,
            target_kind,
            target_id,
            Some(response_locale.as_str()),
        )
        .await?
        .ok_or(SeoError::NotFound)
    }

    async fn prepare_meta_transition(
        &self,
        tenant: &TenantContext,
        input: &SeoMetaInput,
    ) -> SeoResult<String> {
        let response_locale = upsert_response_locale(input, tenant.default_locale.as_str())?;

        if self
            .load_target_state(
                tenant,
                input.target_kind.clone(),
                input.target_id,
                tenant.default_locale.as_str(),
            )
            .await?
            .is_none()
        {
            return Err(SeoError::NotFound);
        }

        let settings = self.load_settings(tenant.id).await?;
        if let Some(canonical_url) = input.canonical_url.as_deref() {
            validate_target_url(
                canonical_url,
                settings.allowed_canonical_hosts.as_slice(),
                "canonical_url",
            )?;
        }
        if let Some(structured_data) = input.structured_data.as_ref() {
            validate_structured_data_payload(&structured_data.0)?;
        }

        Ok(response_locale)
    }

    async fn persist_meta_transition_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant: &TenantContext,
        input: SeoMetaInput,
        response_locale: &str,
        transition_ref: Option<&str>,
    ) -> SeoResult<()> {
        let target_kind = input.target_kind.clone();
        let target_id = input.target_id;
        let existing = seo_meta::Entity::find()
            .filter(seo_meta::Column::TenantId.eq(tenant.id))
            .filter(seo_meta::Column::TargetType.eq(input.target_kind.as_str()))
            .filter(seo_meta::Column::TargetId.eq(input.target_id))
            .one(txn)
            .await?;

        let meta = if let Some(existing) = existing {
            let mut active: seo_meta::ActiveModel = existing.into();
            active.no_index = Set(input.noindex);
            active.no_follow = Set(input.nofollow);
            active.canonical_url = Set(input.canonical_url.clone());
            active.structured_data = Set(input.structured_data.clone().map(|value| value.0));
            active.update(txn).await?
        } else {
            seo_meta::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.id),
                target_type: Set(input.target_kind.as_str().to_string()),
                target_id: Set(input.target_id),
                no_index: Set(input.noindex),
                no_follow: Set(input.nofollow),
                canonical_url: Set(input.canonical_url.clone()),
                structured_data: Set(input.structured_data.clone().map(|value| value.0)),
            }
            .insert(txn)
            .await?
        };

        for translation in input.translations {
            let locale = super::normalize_effective_locale(
                translation.locale.as_str(),
                tenant.default_locale.as_str(),
            )?;
            let existing_translation = meta_translation::Entity::find()
                .filter(meta_translation::Column::MetaId.eq(meta.id))
                .filter(meta_translation::Column::Locale.eq(locale.clone()))
                .one(txn)
                .await?;

            if let Some(existing_translation) = existing_translation {
                let mut active: meta_translation::ActiveModel = existing_translation.into();
                active.title = Set(trimmed_option(translation.title));
                active.description = Set(trimmed_option(translation.description));
                active.keywords = Set(trimmed_option(translation.keywords));
                active.og_title = Set(trimmed_option(translation.og_title));
                active.og_description = Set(trimmed_option(translation.og_description));
                active.og_image = Set(trimmed_option(translation.og_image));
                active.update(txn).await?;
            } else {
                meta_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    meta_id: Set(meta.id),
                    locale: Set(locale),
                    title: Set(trimmed_option(translation.title)),
                    description: Set(trimmed_option(translation.description)),
                    keywords: Set(trimmed_option(translation.keywords)),
                    og_title: Set(trimmed_option(translation.og_title)),
                    og_description: Set(trimmed_option(translation.og_description)),
                    og_image: Set(trimmed_option(translation.og_image)),
                }
                .insert(txn)
                .await?;
            }
        }

        self.publish_seo_meta_upserted_event_in_tx(
            txn,
            tenant.id,
            target_kind.as_str(),
            target_id,
            response_locale,
            "explicit",
            transition_ref,
        )
        .await
    }

''',
    "metadata transition helper",
)
meta = replace_between(
    meta,
    "    pub async fn rollback_revision(",
    "    async fn load_explicit_meta_in_tx(",
    '''    pub async fn rollback_revision(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        revision: i32,
    ) -> SeoResult<SeoMetaRecord> {
        let Some(snapshot) = seo_revision::Entity::find()
            .filter(seo_revision::Column::TenantId.eq(tenant.id))
            .filter(seo_revision::Column::TargetKind.eq(target_kind.as_str()))
            .filter(seo_revision::Column::TargetId.eq(target_id))
            .filter(seo_revision::Column::Revision.eq(revision))
            .one(&self.db)
            .await?
        else {
            return Err(SeoError::NotFound);
        };

        let transition_ref = format!("revision:{revision}");
        let kind = target_kind.clone();
        let input = snapshot_to_input(snapshot.payload, target_kind, target_id);
        let response_locale = self.prepare_meta_transition(tenant, &input).await?;
        let txn = self.db.begin().await?;

        self.persist_meta_transition_in_tx(
            &txn,
            tenant,
            input,
            response_locale.as_str(),
            Some(transition_ref.as_str()),
        )
        .await?;
        self.publish_seo_revision_rolled_back_event_in_tx(
            &txn,
            tenant.id,
            kind.as_str(),
            target_id,
            revision,
        )
        .await?;
        txn.commit().await?;

        self.seo_meta(
            tenant,
            kind,
            target_id,
            Some(response_locale.as_str()),
        )
        .await?
        .ok_or(SeoError::NotFound)
    }

''',
    "revision rollback transaction",
)
meta_path.write_text(meta)


events_path = Path("crates/rustok-seo/src/services/events.rs")
events = events_path.read_text()
events = replace_between(
    events,
    "    pub(super) async fn publish_seo_revision_rolled_back_event(",
    "    #[allow(clippy::too_many_arguments)]",
    '''    pub(super) async fn publish_seo_revision_rolled_back_event_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        revision: i32,
    ) -> SeoResult<()> {
        let event_type = "seo.revision.rolled_back";
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

        let event = DomainEvent::SeoRevisionRolledBack {
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
                    "failed to enqueue SEO revision rollback event transactionally: {error}"
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

''',
    "revision rollback event transaction",
)
events_path.write_text(events)


test_path = Path("crates/rustok-seo/tests/meta_transaction.rs")
test = test_path.read_text()
test = replace_between(
    test,
    "struct FailOnSecondTransport {",
    "struct TestPageProvider;",
    '''struct FailOnNthTransport {
    calls: AtomicUsize,
    fail_on: usize,
}

impl FailOnNthTransport {
    fn new(fail_on: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail_on,
        }
    }
}

#[async_trait]
impl EventTransport for FailOnNthTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(Error::Validation(format!(
                "forced SEO transaction failure on publish {call}"
            )));
        }
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

''',
    "parameterized failing transport",
)
test = test.replace(
    "FailOnSecondTransport::new()",
    "FailOnNthTransport::new(2)",
)
test = replace_once(
    test,
    "    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,\n    EntityTrait, PaginatorTrait, Statement,",
    "    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database,\n    DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Statement,",
    "test query imports",
)
test = replace_once(
    test,
    "fn page_slug() -> SeoTargetSlug {",
    '''#[tokio::test]
async fn revision_rollback_rolls_back_when_rollback_reindex_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "SEO rollback tenant".to_string(),
        slug: "seo-rollback".to_string(),
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
    .expect("current metadata should be seeded");
    meta_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        meta_id: Set(meta_id),
        locale: Set("en".to_string()),
        title: Set(Some("Current title".to_string())),
        description: Set(Some("Current description".to_string())),
        keywords: Set(None),
        og_title: Set(None),
        og_description: Set(None),
        og_image: Set(None),
    }
    .insert(&db)
    .await
    .expect("current metadata translation should be seeded");
    seo_revision::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant.id),
        target_kind: Set(page_slug().into_string()),
        target_id: Set(target_id),
        revision: Set(1),
        note: Set(Some("Rollback snapshot".to_string())),
        payload: Set(json!({
            "noindex": true,
            "nofollow": true,
            "canonical_url": null,
            "structured_data": null,
            "translations": [{
                "locale": "en",
                "title": "Revision title",
                "description": "Revision description",
                "keywords": null,
                "og_title": null,
                "og_description": null,
                "og_image": null
            }]
        })),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(&db)
    .await
    .expect("rollback revision should be seeded");

    let mut registry = SeoTargetRegistry::default();
    registry
        .register(TestPageProvider)
        .expect("test page provider should register");
    let service = SeoService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailOnNthTransport::new(4))),
        Arc::new(registry),
    );
    let error = service
        .rollback_revision(&tenant, page_slug(), target_id, 1)
        .await
        .expect_err("rollback reindex failure must abort the whole revision rollback");
    assert!(
        error
            .to_string()
            .contains("failed to enqueue SEO entity reindex event transactionally")
    );

    let current_meta = seo_meta::Entity::find_by_id(meta_id)
        .one(&db)
        .await
        .expect("metadata should load")
        .expect("metadata should remain");
    assert!(!current_meta.no_index);
    assert!(!current_meta.no_follow);
    let current_translation = meta_translation::Entity::find()
        .filter(meta_translation::Column::MetaId.eq(meta_id))
        .filter(meta_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("translation should load")
        .expect("translation should remain");
    assert_eq!(current_translation.title.as_deref(), Some("Current title"));
    assert_eq!(
        current_translation.description.as_deref(),
        Some("Current description")
    );
    assert_eq!(
        seo_revision::Entity::find()
            .count(&db)
            .await
            .expect("revision count should load"),
        1
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

fn page_slug() -> SeoTargetSlug {''',
    "revision rollback regression test",
)
test_path.write_text(test)


roadmap_path = Path("docs/roadmaps/seo-hardening-progress.md")
roadmap = roadmap_path.read_text()
roadmap = replace_once(
    roadmap,
    "- [ ] Persist revision rollback and all resulting events transactionally.",
    "- [x] Persist revision rollback and all resulting events transactionally. (revision rollback PR)",
    "revision rollback roadmap item",
)
roadmap = replace_once(
    roadmap,
    "- [ ] Add rollback coverage for metadata and revision transactions. Metadata and revision creation rollback are covered; revision rollback remains open. (#2056, #2059)",
    "- [x] Add rollback coverage for metadata and revision transactions. (#2056, #2059, revision rollback PR)",
    "revision rollback coverage roadmap item",
)
roadmap_path.write_text(roadmap)
