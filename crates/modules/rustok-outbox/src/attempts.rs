//! Relay-owned durable publication intent and outcome history.
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "outbox_dispatch_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub event_id: Uuid,
    pub claim_token: String,
    pub started_at: DateTimeUtc,
    pub finished_at: Option<DateTimeUtc>,
    pub state: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub(crate) const OPEN: &str = "open";
pub(crate) const CONFIRMED: &str = "confirmed";
pub(crate) const UNCERTAIN: &str = "uncertain";
pub(crate) const NOT_STARTED: &str = "not_started";
