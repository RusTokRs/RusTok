use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum NotificationState {
    #[sea_orm(string_value = "unread")]
    Unread,
    #[sea_orm(string_value = "seen")]
    Seen,
    #[sea_orm(string_value = "read")]
    Read,
    #[sea_orm(string_value = "archived")]
    Archived,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum NotificationPriorityValue {
    #[sea_orm(string_value = "low")]
    Low,
    #[sea_orm(string_value = "normal")]
    Normal,
    #[sea_orm(string_value = "high")]
    High,
    #[sea_orm(string_value = "urgent")]
    Urgent,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    #[sea_orm(string_value = "in_app")]
    InApp,
    #[sea_orm(string_value = "email")]
    Email,
    #[sea_orm(string_value = "web_push")]
    WebPush,
    #[sea_orm(string_value = "mobile_push")]
    MobilePush,
    #[sea_orm(string_value = "sms")]
    Sms,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "leased")]
    Leased,
    #[sea_orm(string_value = "sent")]
    Sent,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "permanent_error")]
    PermanentError,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
#[serde(rename_all = "snake_case")]
pub enum NotificationJobStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "leased")]
    Leased,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "dead_letter")]
    DeadLetter,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
#[serde(rename_all = "snake_case")]
pub enum NotificationSourceInboxStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "processing")]
    Processing,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "suppressed")]
    Suppressed,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum FanoutItemStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "processed")]
    Processed,
    #[sea_orm(string_value = "skipped")]
    Skipped,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum NotificationDeliveryMode {
    #[sea_orm(string_value = "off")]
    Off,
    #[sea_orm(string_value = "instant")]
    Instant,
    #[sea_orm(string_value = "digest")]
    Digest,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum DigestMode {
    #[sea_orm(string_value = "hourly")]
    Hourly,
    #[sea_orm(string_value = "daily")]
    Daily,
    #[sea_orm(string_value = "weekly")]
    Weekly,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(24))")]
#[serde(rename_all = "snake_case")]
pub enum DigestJobStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "leased")]
    Leased,
    #[sea_orm(string_value = "ready")]
    Ready,
    #[sea_orm(string_value = "sent")]
    Sent,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "dead_letter")]
    DeadLetter,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum PushPlatform {
    #[sea_orm(string_value = "web")]
    Web,
    #[sea_orm(string_value = "ios")]
    Ios,
    #[sea_orm(string_value = "android")]
    Android,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(16))")]
#[serde(rename_all = "snake_case")]
pub enum PushSubscriptionStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "revoked")]
    Revoked,
}
