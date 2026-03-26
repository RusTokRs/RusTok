//! # RusToK Server Tasks
//!
//! Background tasks for maintenance and operations.
//! Run with: `cargo loco task --name <task_name>`

use loco_rs::task::Tasks;

mod cleanup;
mod create_oauth_app;
mod db_baseline;
mod media_cleanup;
#[cfg(feature = "mod-profiles")]
mod profiles_backfill;
mod rebuild;

/// Register all available tasks
pub fn register(tasks: &mut Tasks) {
    // Maintenance tasks
    tasks.register(cleanup::CleanupTask);
    tasks.register(create_oauth_app::CreateOAuthAppTask);
    tasks.register(db_baseline::DbBaselineTask);
    tasks.register(media_cleanup::MediaCleanupTask);
    #[cfg(feature = "mod-profiles")]
    tasks.register(profiles_backfill::ProfilesBackfillTask);
    tasks.register(rebuild::RebuildTask);
}
