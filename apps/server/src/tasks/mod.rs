//! # RusToK Server Tasks
//!
//! Background tasks for maintenance and operations.
//! Current legacy server task bridge; target entrypoints belong to `rustok-cli`.

pub type TaskAppContext = loco_rs::app::AppContext;
pub use loco_rs::task::{Task, TaskInfo, Tasks, Vars};

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
