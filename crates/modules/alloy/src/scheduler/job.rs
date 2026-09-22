use chrono::{DateTime, Utc};

use crate::model::ScriptId;

#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub script_id: ScriptId,
    pub script_name: String,
    pub cron_expression: String,
    pub next_run: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
    pub running: bool,
}
