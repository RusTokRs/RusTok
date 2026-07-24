impl SeoService {
    pub(super) async fn execute_next_bulk_job_with_bounded_io(
        &self,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        let running = seo_bulk_job::Entity::find()
            .filter(seo_bulk_job::Column::Status.eq(SeoBulkJobStatus::Running.as_str()))
            .filter(seo_bulk_job::Column::OperationKind.is_in([
                SeoBulkJobOperationKind::Apply.as_str(),
                SeoBulkJobOperationKind::ExportCsv.as_str(),
                SeoBulkJobOperationKind::ImportCsv.as_str(),
            ]))
            .order_by_asc(seo_bulk_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let running = if let Some(job) = running {
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
            Some(SeoBulkJobOperationKind::Apply) => self.execute_apply_job_chunk(&running).await,
            Some(SeoBulkJobOperationKind::ExportCsv) => {
                self.execute_export_job_chunk_compat(&running).await
            }
            Some(SeoBulkJobOperationKind::ImportCsv) => {
                match self.normalize_bulk_import_job_payload(&running).await {
                    Ok(normalized) => self.execute_import_job_chunk(&normalized).await,
                    Err(error) => Err(error),
                }
            }
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

    async fn execute_export_job_chunk_compat(
        &self,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<()> {
        let tenant = self.load_tenant_context(job.tenant_id).await?;
        let payload = self.decode_bounded_export_payload(&tenant, job).await?;
        if !payload.target_ids.is_empty() {
            return self.execute_export_job_chunk(job).await;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        if progress.artifacts == 0 {
            let filter = normalize_bulk_list_input(
                payload.input.filter,
                tenant.default_locale.as_str(),
            )?;
            let mut writer = WriterBuilder::new()
                .has_headers(false)
                .from_writer(Vec::<u8>::new());
            writer.write_record(CSV_HEADERS).map_err(|error| {
                SeoError::validation(format!(
                    "failed to write empty export CSV header: {error}"
                ))
            })?;
            let bytes = writer.into_inner().map_err(|error| {
                SeoError::validation(format!(
                    "failed to finalize empty export CSV writer: {error}"
                ))
            })?;
            let content = String::from_utf8(bytes).map_err(|error| {
                SeoError::validation(format!(
                    "empty export CSV is not valid UTF-8: {error}"
                ))
            })?;
            self.insert_bulk_job_artifact(
                job,
                "export_csv",
                format!(
                    "seo-bulk-export-{}-{}-{}.csv",
                    filter.target_kind.as_str(),
                    filter.locale,
                    job.id,
                ),
                CSV_MIME_TYPE,
                content,
            )
            .await?;
        }

        let progress = self.load_bulk_job_progress(job.id).await?;
        self.finish_bulk_job(
            job,
            progress.processed,
            progress.succeeded,
            progress.failed,
            progress.artifacts,
            None,
        )
        .await
    }

    async fn normalize_bulk_import_job_payload(
        &self,
        job: &seo_bulk_job::Model,
    ) -> SeoResult<seo_bulk_job::Model> {
        let mut payload = self.decode_bounded_import_payload(job).await?;
        if payload.input.locale == job.locale
            && serde_json::from_value::<QueuedBulkImportPayload>(job.input_payload.clone()).is_ok()
        {
            return Ok(job.clone());
        }

        payload.input.locale = job.locale.clone();
        let now = Utc::now().fixed_offset();
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.input_payload = Set(serde_json::to_value(&payload).map_err(|error| {
            SeoError::validation(format!(
                "failed to normalize bounded bulk import payload: {error}"
            ))
        })?);
        active.updated_at = Set(now);
        active.update(&self.db).await.map_err(Into::into)
    }
}
