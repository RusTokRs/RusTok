impl SharedCommentsTcpDelegationScheduleHandle {
    pub(super) fn from_prepared_file(
        file_path: PathBuf,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<Self, String> {
        let snapshot = build_schedule_snapshot(
            schedule,
            generation,
            keyring::CommentsTcpDelegationKeyringSource::File,
        )?;
        Ok(Self::new(
            DelegationScheduleSource::File(file_path),
            snapshot,
        ))
    }

    pub(super) fn replace_prepared_with_commit<F>(
        &self,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
        source: keyring::CommentsTcpDelegationKeyringSource,
        before_publish: F,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String>
    where
        F: FnOnce() -> std::result::Result<(), String>,
    {
        let configured_source = match &self.0.source {
            DelegationScheduleSource::HostProvided => {
                keyring::CommentsTcpDelegationKeyringSource::HostProvided
            }
            DelegationScheduleSource::File(_) => keyring::CommentsTcpDelegationKeyringSource::File,
        };
        if source != configured_source {
            return self.reject(
                "Comments TCP persisted delegation schedule cannot change source category"
                    .to_string(),
            );
        }
        let candidate = match build_schedule_snapshot(schedule, generation, source) {
            Ok(candidate) => candidate,
            Err(error) => return self.reject(error),
        };
        self.replace_candidate_with_commit(candidate, before_publish)
    }

    fn replace_candidate_with_commit<F>(
        &self,
        candidate: DelegationScheduleSnapshot,
        before_publish: F,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String>
    where
        F: FnOnce() -> std::result::Result<(), String>,
    {
        let now_ms = current_unix_ms()?;
        let mut current = match self.0.current.write() {
            Ok(current) => current,
            Err(_) => {
                return self
                    .reject("Comments TCP delegation schedule state is unavailable".to_string());
            }
        };
        if candidate.source != current.source {
            drop(current);
            return self.reject(
                "Comments TCP delegation schedule reload cannot change source category".to_string(),
            );
        }
        if candidate.generation <= current.generation {
            drop(current);
            return self.reject(
                "Comments TCP delegation schedule generation must be greater than the active generation"
                    .to_string(),
            );
        }
        if let Err(error) = candidate
            .schedule
            .validate_replacement_from(&current.schedule, now_ms)
        {
            drop(current);
            return self.reject(format!(
                "Comments TCP delegation schedule replacement is unsafe: {error}"
            ));
        }
        candidate.schedule.current_keyring_at(now_ms).map_err(|_| {
            "Comments TCP delegation schedule replacement has no active signing key".to_string()
        })?;

        let previous_generation = current.generation;
        let current_selection = schedule_selection_at(&candidate, now_ms)?;
        if let Err(error) = before_publish() {
            drop(current);
            return self.reject(error);
        }

        *current = candidate;
        self.0.successful_reloads.fetch_add(1, Ordering::Relaxed);
        Ok(CommentsTcpDelegationScheduleReloadOutcome {
            previous_generation,
            current: current_selection,
        })
    }
}
