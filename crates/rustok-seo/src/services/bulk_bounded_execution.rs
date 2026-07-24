const BULK_APPLY_CHUNK_SIZE: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueuedBulkApplyPayload {
    input: SeoBulkApplyInput,
    target_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy)]
struct BulkJobProgress {
    processed: i32,
    succeeded: i32,
    failed: i32,
    artifacts: i32,
}

#[derive(Debug, Clone)]
struct BatchedBulkSelectionResolution {
    filter: NormalizedBulkListFilter,
    rows: Vec<super::bulk_read_model::BulkReadRow>,
}

impl SeoService {
    pub(super) async fn preview_bulk_selection_count_batched(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<SeoBulkSelectionPreviewRecord> {
        let resolution = self
            .resolve_bulk_selection_batched(tenant, selection)
            .await?;
        Ok(SeoBulkSelectionPreviewRecord {
            count: resolution.rows.len() as i32,
        })
    }

    pub(super) async fn queue_bulk_apply_batched(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkApplyInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        validate_bulk_apply(&input)?;
        let resolution = self
            .resolve_bulk_selection_batched(tenant, input.selection.clone())
            .await?;
        let target_ids = resolution
            .rows
            .iter()
            .map(|row| row.target_id)
            .collect::<Vec<_>>();
        let queued_payload = QueuedBulkApplyPayload {
            input: input.clone(),
            target_ids: target_ids.clone(),
        };
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::Apply.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(resolution.filter.target_kind.as_str().to_string()),
            locale: Set(resolution.filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&resolution.filter).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk filter: {err}"))
            })?),
            input_payload: Set(serde_json::to_value(&queued_payload).map_err(|err| {
                SeoError::validation(format!("failed to serialize bounded bulk apply payload: {err}"))
            })?),
            publish_after_write: Set(input.publish_after_write),
            matched_count: Set(target_ids.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub(super) async fn queue_bulk_export_batched(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkExportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        let filter =
            normalize_bulk_list_input(input.filter.clone(), tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        let now = Utc::now().fixed_offset();
        let model = seo_bulk_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            operation_kind: Set(SeoBulkJobOperationKind::ExportCsv.as_str().to_string()),
            status: Set(SeoBulkJobStatus::Queued.as_str().to_string()),
            target_kind: Set(filter.target_kind.as_str().to_string()),
            locale: Set(filter.locale.clone()),
            filter_payload: Set(serde_json::to_value(&filter).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk filter: {err}"))
            })?),
            input_payload: Set(serde_json::to_value(&input).map_err(|err| {
                SeoError::validation(format!("failed to serialize bulk export input: {err}"))
            })?),
            publish_after_write: Set(false),
            matched_count: Set(rows.len() as i32),
            processed_count: Set(0),
            succeeded_count: Set(0),
            failed_count: Set(0),
            artifact_count: Set(0),
            last_error: Set(None),
            created_by: Set(created_by),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.bulk_job(tenant.id, model.id)
            .await?
            .ok_or(SeoError::NotFound)
    }

    pub(super) async fn execute_next_bulk_job_batched(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        let running_apply = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
            .filter(
                seo_bulk_job::Column::OperationKind.eq(SeoBulkJobOperationKind::Apply.as_str()),
            )
            .order_by_asc(seo_bulk_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let running = if let Some(job) = running_apply {
            job
        } else {
            let Some(job) = seo_bulk_job::Entity::find()
                .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Queued.as_str()))
                .order_by_asc(seo_bulk_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let now = Utc::now().fixed_offset();
            let mut active: seo_bulk_job::ActiveModel = job.into();
            active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
            active.started_at = Set(Some(now));
            active.updated_at = Set(now);
            active.last_error = Set(None);
            active.update(&self.db).await?
        };

        let result = match SeoBulkJobOperationKind::parse(running.operation_kind.as_str()) {
            Some(SeoBulkJobOperationKind::Apply) => {
                self.execute_apply_job_chunk(&running).await
            }
            Some(SeoBulkJobOperationKind::ExportCsv) => {
                self.execute_export_job_batched(&running).await
            }
            Some(SeoBulkJobOperationKind::ImportCsv) => self.execute_import_job(&running).await,
            None => Err(SeoError::validation(format!(
                "unknown bulk operation kind `{}`",
                running.operation_kind
            ))),
        };

        if let Err(error) = result {
            self.fail_bulk_job(&running, error.to_string()).await?;
        }

        self.bulk_job(running.tenant_id, running.id).await
    }

    async fn resolve_bulk_selection_batched(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<BatchedBulkSelectionResolution> {
        let filter = selection
            .filter
            .ok_or_else(|| SeoError::validation("bulk selection filter is required"))?;
        let filter = normalize_bulk_list_input(filter, tenant.default_locale.as_str())?;
        let mut rows = self.collect_bulk_rows_for_filter(tenant, &filter).await?;
        if selection.mode == SeoBulkSelectionMode::SelectedIds {
            let selected = selection.selected_ids.into_iter().collect::<HashSet<_>>();
            rows.retain(|row| selected.contains(&row.target_id));
        }

        Ok(BatchedBulkSelectionResolution { filter, rows })
    }

    async fn collect_bulk_rows_for_filter(
        &self,
        tenant: &TenantContext,
        filter: &NormalizedBulkListFilter,
    ) -> SeoResult<Vec<super::bulk_read_model::BulkReadRow>> {
        self.collect_bulk_read_rows(
            tenant,
            &super::bulk_read_model::BulkReadFilter {
                target_kind: filter.target_kind.clone(),
                locale: filter.locale.clone(),
                query: filter.query.clone(),
                source: filter.source,
            },
        )
        .await
    }

    async fn execute_apply_job_chunk(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let payload = self.decode_bounded_apply_payload(&tenant, job).await?;
        let processed_items = seo_bulk_job_item::Entity::find()
            .filter(seo_bulk_job_item::Column::JobId.eq(job.id))
            .order_by_asc(seo_bulk_job_item::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let processed_ids = processed_items
            .iter()
            .map(|item| item.target_id)
            .collect::<HashSet<_>>();
        let chunk = payload
            .target_ids
            .iter()
            .copied()
            .filter(|target_id| !processed_ids.contains(target_id))
            .take(BULK_APPLY_CHUNK_SIZE)
            .collect::<Vec<_>>();

        if chunk.is_empty() {
            let progress = self.load_bulk_job_progress(job.id).await?;
            return self
                .finish_bulk_job(
                    job,
                    progress.processed,
                    progress.succeeded,
                    progress.failed,
                    progress.artifacts,
                    None,
                )
                .await;
        }

        let chunk_start = processed_ids.len() + 1;
        let chunk_end = chunk_start + chunk.len() - 1;
        let mut preview_rows = Vec::<Vec<String>>::new();
        let mut failure_rows = Vec::new();

        if payload.input.apply_mode == SeoBulkApplyMode::PreviewOnly {
            let resolution = self
                .resolve_bulk_selection_batched(&tenant, payload.input.selection.clone())
                .await?;
            let projections = resolution
                .rows
                .into_iter()
                .map(|row| (row.target_id, row.projection))
                .collect::<HashMap<_, _>>();

            for target_id in chunk {
                if let Some(projection) = projections.get(&target_id) {
                    preview_rows.push(export_bulk_projection_row(
                        resolution.filter.target_kind.clone(),
                        target_id,
                        resolution.filter.locale.as_str(),
                        projection,
                    ));
                    self.insert_bulk_job_item(job, target_id, None, None)
                        .await?;
                } else {
                    let message = "SEO target not found".to_string();
                    self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                        .await?;
                    failure_rows.push((
                        preview_failure_row(
                            resolution.filter.target_kind.as_str(),
                            target_id,
                            resolution.filter.locale.as_str(),
                        ),
                        message,
                    ));
                }
            }
        } else {
            let filter = normalize_bulk_list_input(
                payload
                    .input
                    .selection
                    .filter
                    .clone()
                    .ok_or_else(|| SeoError::validation("bulk selection filter is required"))?,
                tenant.default_locale.as_str(),
            )?;
            for target_id in chunk {
                match self
                    .apply_bulk_patch_to_target(
                        &tenant,
                        job.id,
                        filter.target_kind.clone(),
                        filter.locale.as_str(),
                        target_id,
                        &payload.input.patch,
                        payload.input.apply_mode,
                        job.publish_after_write,
                    )
                    .await
                {
                    Ok(revision) => {
                        self.insert_bulk_job_item(job, target_id, None, revision)
                            .await?;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        self.insert_bulk_job_item(job, target_id, Some(message.clone()), None)
                            .await?;
                        failure_rows.push((
                            empty_csv_row(
                                filter.target_kind.as_str(),
                                target_id,
                                filter.locale.as_str(),
                            ),
                            message,
                        ));
                    }
                }
            }
        }

        if !preview_rows.is_empty() {
            let content = build_preview_csv(&preview_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "preview_report",
                format!(
                    "seo-bulk-preview-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }
        if !failure_rows.is_empty() {
            let content = build_failure_csv(&failure_rows)?;
            self.insert_bulk_job_artifact(
                job,
                "failure_report",
                format!(
                    "seo-bulk-apply-failures-{}-{}-{}.csv",
                    job.id, chunk_start, chunk_end
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        if progress.processed as usize >= payload.target_ids.len() {
            self.finish_bulk_job(
                job,
                progress.processed,
                progress.succeeded,
                progress.failed,
                progress.artifacts,
                None,
            )
            .await
        } else {
            self.checkpoint_bulk_apply_job(job, payload.target_ids.len(), progress)
                .await
        }
    }

    async fn decode_bounded_apply_payload(
        &self,
        tenant: &TenantContext,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<QueuedBulkApplyPayload> {
        if let Ok(payload) =
            serde_json::from_value::<QueuedBulkApplyPayload>(job.input_payload.clone())
        {
            return Ok(payload);
        }

        let input = serde_json::from_value::<SeoBulkApplyInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk apply payload: {err}"))
            })?;
        let resolution = self
            .resolve_bulk_selection_batched(tenant, input.selection.clone())
            .await?;
        Ok(QueuedBulkApplyPayload {
            input,
            target_ids: resolution.rows.into_iter().map(|row| row.target_id).collect(),
        })
    }

    async fn load_bulk_job_progress(&self, job_id: Uuid) -> SeoResult<BulkJobProgress> {
        let items = seo_bulk_job_item::Entity::find()
            .filter(seo_bulk_job_item::Column::JobId.eq(job_id))
            .all(&self.db)
            .await?;
        let succeeded = items
            .iter()
            .filter(|item| item.status == "completed")
            .count() as i32;
        let failed = items
            .iter()
            .filter(|item| item.status == "failed")
            .count() as i32;
        let artifacts = seo_bulk_job_artifact::Entity::find()
            .filter(seo_bulk_job_artifact::Column::JobId.eq(job_id))
            .all(&self.db)
            .await?
            .len() as i32;
        Ok(BulkJobProgress {
            processed: items.len() as i32,
            succeeded,
            failed,
            artifacts,
        })
    }

    async fn checkpoint_bulk_apply_job(
        &self,
        job: &seo_bulk_job::Model,
        matched_count: usize,
        progress: BulkJobProgress,
    ) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Running.as_str().to_string());
        active.matched_count = Set(matched_count as i32);
        active.processed_count = Set(progress.processed);
        active.succeeded_count = Set(progress.succeeded);
        active.failed_count = Set(progress.failed);
        active.artifact_count = Set(progress.artifacts);
        active.last_error = Set(None);
        active.completed_at = Set(None);
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }

    async fn execute_export_job_batched(&self, job: &seo_bulk_job::Model) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let input = serde_json::from_value::<SeoBulkExportInput>(job.input_payload.clone())
            .map_err(|err| {
                SeoError::validation(format!("failed to decode bulk export payload: {err}"))
            })?;
        let filter = normalize_bulk_list_input(input.filter, tenant.default_locale.as_str())?;
        let rows = self.collect_bulk_rows_for_filter(&tenant, &filter).await?;
        let mut writer = WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::<u8>::new());
        writer.write_record(CSV_HEADERS).map_err(|err| {
            SeoError::validation(format!("failed to write export CSV header: {err}"))
        })?;

        for row in &rows {
            writer
                .write_record(export_bulk_projection_row(
                    filter.target_kind.clone(),
                    row.target_id,
                    filter.locale.as_str(),
                    &row.projection,
                ))
                .map_err(|err| {
                    SeoError::validation(format!(
                        "failed to serialize export row for {}: {err}",
                        row.target_id
                    ))
                })?;
            self.insert_bulk_job_item(job, row.target_id, None, None)
                .await?;
        }

        let bytes = writer.into_inner().map_err(|err| {
            SeoError::validation(format!("failed to finalize export CSV writer: {err}"))
        })?;
        let content = String::from_utf8(bytes)
            .map_err(|err| SeoError::validation(format!("export CSV is not valid UTF-8: {err}")))?;
        self.insert_bulk_job_artifact(
            job,
            "export_csv",
            format!(
                "seo-bulk-export-{}-{}-{}.csv",
                filter.target_kind.as_str(),
                filter.locale,
                job.id
            ),
            CSV_MIME_TYPE,
            content,
        )
        .await?;

        self.finish_bulk_job(job, rows.len() as i32, rows.len() as i32, 0, 1, None)
            .await
    }
}

fn export_bulk_projection_row(
    target_kind: SeoTargetSlug,
    target_id: Uuid,
    locale: &str,
    projection: &super::bulk_read_model::BulkReadProjection,
) -> Vec<String> {
    vec![
        target_kind.as_str().to_string(),
        target_id.to_string(),
        locale.to_string(),
        projection.title.clone().unwrap_or_default(),
        projection.description.clone().unwrap_or_default(),
        projection.keywords.clone().unwrap_or_default(),
        projection.canonical_url.clone().unwrap_or_default(),
        projection.og_title.clone().unwrap_or_default(),
        projection.og_description.clone().unwrap_or_default(),
        projection.og_image.clone().unwrap_or_default(),
        projection
            .structured_data
            .as_ref()
            .map(Value::to_string)
            .unwrap_or_default(),
        projection.noindex.to_string(),
        projection.nofollow.to_string(),
    ]
}

#[cfg(test)]
mod bounded_execution_tests {
    use super::*;

    #[test]
    fn projection_csv_row_preserves_public_column_order() {
        let target_kind = SeoTargetSlug::new("page").expect("valid target kind");
        let target_id = Uuid::new_v4();
        let row = export_bulk_projection_row(
            target_kind,
            target_id,
            "en-US",
            &super::bulk_read_model::BulkReadProjection {
                effective_locale: "en-US".to_string(),
                source: SeoBulkSource::Explicit,
                title: Some("Title".to_string()),
                description: Some("Description".to_string()),
                keywords: Some("keywords".to_string()),
                canonical_url: Some("/canonical".to_string()),
                og_title: Some("OG Title".to_string()),
                og_description: Some("OG Description".to_string()),
                og_image: Some("https://example.test/image.jpg".to_string()),
                structured_data: Some(json!({"@type": "WebPage"})),
                noindex: true,
                nofollow: false,
            },
        );

        assert_eq!(row.len(), CSV_HEADERS.len());
        assert_eq!(row[1], target_id.to_string());
        assert_eq!(row[10], "{\"@type\":\"WebPage\"}");
        assert_eq!(row[11], "true");
        assert_eq!(row[12], "false");
    }

    #[test]
    fn apply_chunk_size_stays_bounded() {
        assert_eq!(BULK_APPLY_CHUNK_SIZE, 50);
    }
}
