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
    "use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};",
    "use sea_orm::{\n    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait,\n};",
    "meta transaction import",
)
meta_start = "    pub(super) async fn upsert_meta_with_transition("
meta_end = "    pub async fn publish_revision("
meta_replacement = '''    pub(super) async fn upsert_meta_with_transition(
        &self,
        tenant: &TenantContext,
        input: SeoMetaInput,
        transition_ref: Option<String>,
    ) -> SeoResult<SeoMetaRecord> {
        let response_locale = upsert_response_locale(&input, tenant.default_locale.as_str())?;

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

        let target_kind = input.target_kind.clone();
        let target_id = input.target_id;
        let txn = self.db.begin().await?;

        let existing = seo_meta::Entity::find()
            .filter(seo_meta::Column::TenantId.eq(tenant.id))
            .filter(seo_meta::Column::TargetType.eq(input.target_kind.as_str()))
            .filter(seo_meta::Column::TargetId.eq(input.target_id))
            .one(&txn)
            .await?;

        let meta = if let Some(existing) = existing {
            let mut active: seo_meta::ActiveModel = existing.into();
            active.no_index = Set(input.noindex);
            active.no_follow = Set(input.nofollow);
            active.canonical_url = Set(input.canonical_url.clone());
            active.structured_data = Set(input.structured_data.clone().map(|value| value.0));
            active.update(&txn).await?
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
            .insert(&txn)
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
                .one(&txn)
                .await?;

            if let Some(existing_translation) = existing_translation {
                let mut active: meta_translation::ActiveModel = existing_translation.into();
                active.title = Set(trimmed_option(translation.title));
                active.description = Set(trimmed_option(translation.description));
                active.keywords = Set(trimmed_option(translation.keywords));
                active.og_title = Set(trimmed_option(translation.og_title));
                active.og_description = Set(trimmed_option(translation.og_description));
                active.og_image = Set(trimmed_option(translation.og_image));
                active.update(&txn).await?;
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
                .insert(&txn)
                .await?;
            }
        }

        self.publish_seo_meta_upserted_event_in_tx(
            &txn,
            tenant.id,
            target_kind.as_str(),
            target_id,
            response_locale.as_str(),
            "explicit",
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

'''
meta = replace_between(meta, meta_start, meta_end, meta_replacement, "meta upsert transaction")
meta_path.write_text(meta)


events_path = Path("crates/rustok-seo/src/services/events.rs")
events = events_path.read_text()
events_start = "    pub(super) async fn publish_seo_meta_upserted_event("
events_end = "    pub(super) async fn publish_seo_revision_published_event("
events_replacement = '''    pub(super) async fn publish_seo_meta_upserted_event_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        locale: &str,
        source: &str,
        transition_ref: Option<&str>,
    ) -> SeoResult<()> {
        let event_type = "seo.meta.upserted";
        let idempotency_key = self.build_event_key(
            event_type,
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                locale.to_string(),
                transition_ref.unwrap_or("direct").to_string(),
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

        let event = DomainEvent::SeoMetaUpserted {
            target_kind: target_kind.to_string(),
            target_id,
            locale: locale.to_string(),
            source: source.to_string(),
            idempotency_key: idempotency_key.clone(),
        };
        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, None, event.clone())
            .await
            .map_err(|error| {
                SeoError::Database(DbErr::Custom(format!(
                    "failed to enqueue SEO metadata event transactionally: {error}"
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
            self.publish_meta_reindex_in_tx(
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

    async fn publish_meta_reindex_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        seo_event_type: &str,
        idempotency_key: &str,
        trigger: &SeoIndexReindexTrigger,
        observed_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let existing = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_index_delivery::Column::IdempotencyKey.eq(idempotency_key))
            .filter(seo_index_delivery::Column::TargetType.eq(trigger.target_type.as_str()))
            .filter(
                seo_index_delivery::Column::TargetScopeKey.eq(trigger.target_scope_key.as_str()),
            )
            .one(txn)
            .await?;
        if existing.is_some() {
            self.upsert_index_cursor_in_tx(
                txn,
                tenant_id,
                trigger.target_type.as_str(),
                observed_at,
            )
            .await?;
            return Ok(());
        }

        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(
                txn,
                tenant_id,
                None,
                DomainEvent::ReindexRequested {
                    target_type: trigger.target_type.clone(),
                    target_id: trigger.target_id,
                },
            )
            .await
            .map_err(|error| {
                SeoError::Database(DbErr::Custom(format!(
                    "failed to enqueue SEO metadata reindex event transactionally: {error}"
                )))
            })?;

        seo_index_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            seo_event_type: Set(seo_event_type.to_string()),
            idempotency_key: Set(idempotency_key.to_string()),
            target_type: Set(trigger.target_type.clone()),
            target_id: Set(trigger.target_id),
            target_scope: Set(trigger.target_scope.clone()),
            target_scope_key: Set(trigger.target_scope_key.clone()),
            status: Set(INDEX_DELIVERY_STATUS_SENT.to_string()),
            attempt_count: Set(1),
            outbox_event_id: Set(Some(outbox_event_id)),
            next_attempt_at: Set(None),
            last_error: Set(None),
            dead_lettered_at: Set(None),
            created_at: Set(observed_at),
            updated_at: Set(observed_at),
            dispatched_at: Set(Some(observed_at)),
        }
        .insert(txn)
        .await?;

        self.upsert_index_cursor_in_tx(
            txn,
            tenant_id,
            trigger.target_type.as_str(),
            observed_at,
        )
        .await
    }

    async fn upsert_index_cursor_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        target_type: &str,
        observed_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let existing = seo_index_cursor::Entity::find()
            .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
            .filter(seo_index_cursor::Column::TargetType.eq(target_type))
            .one(txn)
            .await?;

        if let Some(existing) = existing {
            if existing.high_water_mark_at >= observed_at {
                return Ok(());
            }

            let mut active: seo_index_cursor::ActiveModel = existing.into();
            active.high_water_mark_at = Set(observed_at);
            active.updated_at = Set(observed_at);
            active.update(txn).await?;
            return Ok(());
        }

        seo_index_cursor::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_type: Set(target_type.to_string()),
            initial_cursor_at: Set(observed_at),
            high_water_mark_at: Set(observed_at),
            last_repair_cursor_at: Set(None),
            replay_mode: Set(INDEX_CURSOR_REPLAY_MODE_NOT_STARTED.to_string()),
            replay_requested_at: Set(None),
            replay_completed_at: Set(None),
            created_at: Set(observed_at),
            updated_at: Set(observed_at),
        }
        .insert(txn)
        .await?;

        Ok(())
    }

'''
events = replace_between(
    events,
    events_start,
    events_end,
    events_replacement,
    "metadata transactional event publisher",
)
events_path.write_text(events)


roadmap_path = Path("docs/roadmaps/seo-hardening-progress.md")
roadmap = roadmap_path.read_text()
roadmap = replace_once(
    roadmap,
    "- [ ] Persist SEO metadata, translations, delivery tracking, and reindex events transactionally.",
    "- [x] Persist SEO metadata, translations, delivery tracking, and reindex events transactionally. (metadata transaction PR)",
    "metadata roadmap item",
)
roadmap = replace_once(
    roadmap,
    "- [ ] Add rollback coverage for metadata and revision transactions.",
    "- [ ] Add rollback coverage for metadata and revision transactions. Metadata rollback is covered; revision creation and rollback remain open. (metadata transaction PR)",
    "metadata rollback roadmap item",
)
roadmap_path.write_text(roadmap)
