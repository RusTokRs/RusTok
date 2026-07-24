mod index_repair_background_impl {
    use super::*;
    use sea_orm::ActiveValue::Set;
    use sea_orm::{
        ActiveModelTrait as _, ColumnTrait as _, EntityTrait as _, QueryFilter as _,
        QueryOrder as _,
    };

    const INDEX_REPAIR_JOB_QUEUED: &str = "queued";
    const INDEX_REPAIR_JOB_RUNNING: &str = "running";
    const INDEX_REPAIR_JOB_COMPLETED: &str = "completed";
    const INDEX_REPAIR_JOB_FAILED: &str = "failed";
    const INDEX_REPAIR_JOB_MAX_LIMIT: usize = 500;

    mod job_entity {
        use sea_orm::entity::prelude::*;
        use serde::{Deserialize, Serialize};
        use uuid::Uuid;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
        #[sea_orm(table_name = "seo_index_repair_jobs")]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub id: Uuid,
            pub tenant_id: Uuid,
            pub status: String,
            pub target_type: Option<String>,
            pub limit: i32,
            pub replay_historical: bool,
            pub repaired_count: i32,
            pub replayed_count: i32,
            pub historical_events_scanned: i32,
            pub replay_run_id: Option<Uuid>,
            pub last_error: Option<String>,
            pub started_at: Option<DateTimeWithTimeZone>,
            pub completed_at: Option<DateTimeWithTimeZone>,
            pub created_at: DateTimeWithTimeZone,
            pub updated_at: DateTimeWithTimeZone,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    impl SeoService {
        pub(super) async fn queue_index_repair_replay_background(
            &self,
            tenant_id: Uuid,
            target_type: Option<&str>,
            limit: usize,
            replay_historical: bool,
        ) -> SeoResult<crate::dto::SeoIndexRepairReplayResultRecord> {
            let target_type = normalize_background_index_target_type(target_type)?;
            let bounded_limit = limit.clamp(1, INDEX_REPAIR_JOB_MAX_LIMIT);

            let active = job_entity::Entity::find()
                .filter(job_entity::Column::TenantId.eq(tenant_id))
                .filter(
                    job_entity::Column::Status
                        .is_in([INDEX_REPAIR_JOB_QUEUED, INDEX_REPAIR_JOB_RUNNING]),
                )
                .order_by_asc(job_entity::Column::CreatedAt)
                .one(&self.db)
                .await?;
            if let Some(active) = active {
                return Ok(map_background_index_repair_job(&active));
            }

            let now = chrono::Utc::now().fixed_offset();
            let job = job_entity::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                status: Set(INDEX_REPAIR_JOB_QUEUED.to_string()),
                target_type: Set(target_type),
                limit: Set(bounded_limit as i32),
                replay_historical: Set(replay_historical),
                repaired_count: Set(0),
                replayed_count: Set(0),
                historical_events_scanned: Set(0),
                replay_run_id: Set(None),
                last_error: Set(None),
                started_at: Set(None),
                completed_at: Set(None),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&self.db)
            .await?;

            Ok(map_background_index_repair_job(&job))
        }

        pub(super) async fn execute_next_index_repair_replay_job_background(
            &self,
        ) -> SeoResult<Option<crate::dto::SeoIndexRepairReplayResultRecord>> {
            let running = job_entity::Entity::find()
                .filter(job_entity::Column::Status.eq(INDEX_REPAIR_JOB_RUNNING))
                .order_by_asc(job_entity::Column::UpdatedAt)
                .one(&self.db)
                .await?;

            let job = if let Some(running) = running {
                running
            } else {
                let Some(queued) = job_entity::Entity::find()
                    .filter(job_entity::Column::Status.eq(INDEX_REPAIR_JOB_QUEUED))
                    .order_by_asc(job_entity::Column::CreatedAt)
                    .one(&self.db)
                    .await?
                else {
                    return Ok(None);
                };

                let now = chrono::Utc::now().fixed_offset();
                let mut active: job_entity::ActiveModel = queued.into();
                active.status = Set(INDEX_REPAIR_JOB_RUNNING.to_string());
                active.started_at = Set(Some(now));
                active.completed_at = Set(None);
                active.last_error = Set(None);
                active.updated_at = Set(now);
                active.update(&self.db).await?
            };

            let result = self
                .run_index_repair_replay(
                    job.tenant_id,
                    job.target_type.as_deref(),
                    job.limit.clamp(1, INDEX_REPAIR_JOB_MAX_LIMIT as i32) as usize,
                    job.replay_historical,
                )
                .await;

            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    self.fail_background_index_repair_job(&job, error.to_string())
                        .await?;
                    return Err(error);
                }
            };

            let current = job_entity::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?;
            let now = chrono::Utc::now().fixed_offset();
            let mut active: job_entity::ActiveModel = current.into();
            active.status = Set(INDEX_REPAIR_JOB_COMPLETED.to_string());
            active.repaired_count = Set(result.repaired_count);
            active.replayed_count = Set(result.replayed_count);
            active.historical_events_scanned = Set(result.historical_events_scanned);
            active.replay_run_id = Set(result.replay_run_id);
            active.last_error = Set(None);
            active.completed_at = Set(Some(now));
            active.updated_at = Set(now);
            let completed = active.update(&self.db).await?;

            Ok(Some(map_background_index_repair_job(&completed)))
        }

        async fn fail_background_index_repair_job(
            &self,
            job: &job_entity::Model,
            message: String,
        ) -> SeoResult<()> {
            let current = job_entity::Entity::find_by_id(job.id)
                .one(&self.db)
                .await?
                .ok_or(SeoError::NotFound)?;
            let now = chrono::Utc::now().fixed_offset();
            let mut active: job_entity::ActiveModel = current.into();
            active.status = Set(INDEX_REPAIR_JOB_FAILED.to_string());
            active.last_error = Set(Some(rustok_core::truncate(message.trim(), 2048)));
            active.completed_at = Set(Some(now));
            active.updated_at = Set(now);
            active.update(&self.db).await?;
            Ok(())
        }
    }

    fn normalize_background_index_target_type(
        target_type: Option<&str>,
    ) -> SeoResult<Option<String>> {
        let Some(value) = target_type else {
            return Ok(None);
        };
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Ok(None);
        }
        match normalized.as_str() {
            "content" | "product" => Ok(Some(normalized)),
            _ => Err(SeoError::validation(format!(
                "unsupported index target_type `{}`; expected `content` or `product`",
                value.trim()
            ))),
        }
    }

    fn map_background_index_repair_job(
        job: &job_entity::Model,
    ) -> crate::dto::SeoIndexRepairReplayResultRecord {
        let replay_mode = if job.status == INDEX_REPAIR_JOB_COMPLETED {
            if job.replay_historical {
                crate::dto::SeoIndexReplayMode::ReplayCompleted
            } else {
                crate::dto::SeoIndexReplayMode::RepairOnly
            }
        } else {
            crate::dto::SeoIndexReplayMode::NotStarted
        };
        crate::dto::SeoIndexRepairReplayResultRecord {
            target_type: job.target_type.clone(),
            limit: job.limit,
            replay_mode,
            repaired_count: job.repaired_count,
            replayed_count: job.replayed_count,
            historical_events_scanned: job.historical_events_scanned,
            replay_run_id: job.replay_run_id,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn worker_limit_matches_existing_operator_bound() {
            assert_eq!(INDEX_REPAIR_JOB_MAX_LIMIT, 500);
        }

        #[test]
        fn worker_states_are_durable_and_distinct() {
            assert_ne!(INDEX_REPAIR_JOB_QUEUED, INDEX_REPAIR_JOB_RUNNING);
            assert_ne!(INDEX_REPAIR_JOB_RUNNING, INDEX_REPAIR_JOB_COMPLETED);
            assert_ne!(INDEX_REPAIR_JOB_COMPLETED, INDEX_REPAIR_JOB_FAILED);
        }
    }
}
