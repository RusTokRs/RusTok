use sea_orm::entity::prelude::*;

use crate::model::{
    DeliveryStatus, DigestJobStatus, DigestMode, FanoutItemStatus, NotificationChannel,
    NotificationDeliveryMode, NotificationJobStatus, NotificationPriorityValue,
    NotificationState, PushPlatform, PushSubscriptionStatus,
};

pub mod notification {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notifications")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub recipient_id: Uuid,
        pub source_slug: String,
        pub source_event_id: Uuid,
        pub source_revision: i64,
        pub notification_type: String,
        pub template_key: String,
        pub target_owner: String,
        pub target_kind: String,
        pub target_id: Uuid,
        pub actor_id: Option<Uuid>,
        pub priority: NotificationPriorityValue,
        pub state: NotificationState,
        pub template_data_json: Json,
        pub group_key: Option<String>,
        pub idempotency_key: String,
        pub seen_at: Option<DateTimeWithTimeZone>,
        pub read_at: Option<DateTimeWithTimeZone>,
        pub archived_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod delivery_attempt {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_delivery_attempts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub notification_id: Uuid,
        pub recipient_id: Uuid,
        pub channel: NotificationChannel,
        pub status: DeliveryStatus,
        pub provider_key: Option<String>,
        pub idempotency_key: String,
        pub attempt_count: i32,
        pub next_attempt_at: Option<DateTimeWithTimeZone>,
        pub lease_owner: Option<String>,
        pub lease_expires_at: Option<DateTimeWithTimeZone>,
        pub last_error_code: Option<String>,
        pub last_error_message: Option<String>,
        pub provider_message_id: Option<String>,
        pub sent_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod fanout_job {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_fanout_jobs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub source_slug: String,
        pub source_event_id: Uuid,
        pub source_revision: i64,
        pub notification_type: String,
        pub descriptor_json: Json,
        pub audience_cursor: Option<String>,
        pub status: NotificationJobStatus,
        pub attempt_count: i32,
        pub next_attempt_at: Option<DateTimeWithTimeZone>,
        pub lease_owner: Option<String>,
        pub lease_expires_at: Option<DateTimeWithTimeZone>,
        pub last_error_code: Option<String>,
        pub last_error_message: Option<String>,
        pub completed_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod fanout_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_fanout_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub fanout_job_id: Uuid,
        pub recipient_id: Uuid,
        pub status: FanoutItemStatus,
        pub notification_id: Option<Uuid>,
        pub idempotency_key: String,
        pub last_error_code: Option<String>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
        pub processed_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod preference {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_preferences")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub user_id: Uuid,
        pub source_scope: String,
        pub type_scope: String,
        pub delivery_mode: NotificationDeliveryMode,
        pub in_app_enabled: bool,
        pub email_enabled: bool,
        pub push_enabled: bool,
        pub sms_enabled: bool,
        pub digest_mode: DigestMode,
        pub timezone: String,
        pub quiet_start_minute: Option<i16>,
        pub quiet_end_minute: Option<i16>,
        pub revision: i64,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod digest_job {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_digest_jobs")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub recipient_id: Uuid,
        pub schedule_key: String,
        pub digest_mode: DigestMode,
        pub status: DigestJobStatus,
        pub window_start: DateTimeWithTimeZone,
        pub window_end: DateTimeWithTimeZone,
        pub attempt_count: i32,
        pub next_attempt_at: Option<DateTimeWithTimeZone>,
        pub lease_owner: Option<String>,
        pub lease_expires_at: Option<DateTimeWithTimeZone>,
        pub last_error_code: Option<String>,
        pub last_error_message: Option<String>,
        pub sent_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod digest_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_digest_items")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub digest_job_id: Uuid,
        pub notification_id: Uuid,
        pub idempotency_key: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod push_subscription {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "notification_push_subscriptions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: Uuid,
        pub tenant_id: Uuid,
        pub user_id: Uuid,
        pub platform: PushPlatform,
        pub endpoint_hash: String,
        pub encrypted_endpoint: String,
        pub encrypted_p256dh: Option<String>,
        pub encrypted_auth: Option<String>,
        pub key_version: String,
        pub status: PushSubscriptionStatus,
        pub failure_count: i32,
        pub last_success_at: Option<DateTimeWithTimeZone>,
        pub revoked_at: Option<DateTimeWithTimeZone>,
        pub created_at: DateTimeWithTimeZone,
        pub updated_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}
