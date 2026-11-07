use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement, TransactionTrait};
use uuid::Uuid;
use crate::audience::ForumAudienceConstraints;
use crate::error::{ForumError, ForumResult};

/// Ideal helper: one place for the 4× `insert_roles/channels/groups/users` copy-paste
/// across `category_audience*`, `category_moderation_audience`, `topic_audience*`.
/// Tables are `{prefix}_roles|_channels|_groups|_users` with `(tenant_id, subject_id)` PK.
/// `subject_column` is `category_id` or `topic_id` depending on scope — passed by caller
/// so the generic does not need a full EntityTrait bound.
pub async fn insert_roles_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    roles: &[String],
) -> ForumResult<()> {
    if roles.is_empty() { return Ok(()); }
    let mut sql = format!("INSERT INTO {table} (tenant_id, {subject_column}, role) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, role) in roles.iter().enumerate() {
        if i > 0 { sql.push_str(", "); }
        sql.push_str(&format!("(${}, ${}, ${})", vals.len()+1, vals.len()+2, vals.len()+3));
        vals.push(tenant_id.into()); vals.push(subject_id.into()); vals.push(role.clone().into());
    }
    txn.execute(Statement::from_sql_and_values(
        txn.get_database_backend(), sql, vals,
    )).await.map(|_| ()).map_err(ForumError::from)
}

pub async fn insert_channels_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    channels: &[String],
) -> ForumResult<()> {
    if channels.is_empty() { return Ok(()); }
    let mut sql = format!("INSERT INTO {table} (tenant_id, {subject_column}, channel_slug) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, ch) in channels.iter().enumerate() {
        if i > 0 { sql.push_str(", "); }
        sql.push_str(&format!("(${}, ${}, ${})", vals.len()+1, vals.len()+2, vals.len()+3));
        vals.push(tenant_id.into()); vals.push(subject_id.into()); vals.push(ch.clone().into());
    }
    txn.execute(Statement::from_sql_and_values(txn.get_database_backend(), sql, vals)).await.map(|_| ()).map_err(ForumError::from)
}

pub async fn insert_groups_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    groups: &[Uuid],
) -> ForumResult<()> {
    if groups.is_empty() { return Ok(()); }
    let mut sql = format!("INSERT INTO {table} (tenant_id, {subject_column}, group_id) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 { sql.push_str(", "); }
        sql.push_str(&format!("(${}, ${}, ${})", vals.len()+1, vals.len()+2, vals.len()+3));
        vals.push(tenant_id.into()); vals.push(subject_id.into()); vals.push((*g).into());
    }
    txn.execute(Statement::from_sql_and_values(txn.get_database_backend(), sql, vals)).await.map(|_| ()).map_err(ForumError::from)
}

pub async fn insert_users_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    user_ids: &[Uuid],
    effect: &str,
) -> ForumResult<()> {
    if user_ids.is_empty() { return Ok(()); }
    let mut sql = format!("INSERT INTO {table} (tenant_id, {subject_column}, user_id, effect) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, uid) in user_ids.iter().enumerate() {
        if i > 0 { sql.push_str(", "); }
        sql.push_str(&format!("(${}, ${}, ${}, ${})", vals.len()+1, vals.len()+2, vals.len()+3, vals.len()+4));
        vals.push(tenant_id.into()); vals.push(subject_id.into()); vals.push((*uid).into()); vals.push(effect.to_string().into());
    }
    txn.execute(Statement::from_sql_and_values(txn.get_database_backend(), sql, vals)).await.map(|_| ()).map_err(ForumError::from)
}

pub async fn clear_audience_raw(
    txn: &DatabaseTransaction,
    table_prefix: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
) -> ForumResult<()> {
    // Real tables are pluralised: _policies, _roles, _channels, _groups, _users
    for suffix in ["_policies", "_roles", "_channels", "_groups", "_users"] {
        let table = format!("{table_prefix}{suffix}");
        let sql = format!("DELETE FROM {table} WHERE tenant_id = $1 AND {subject_column} = $2");
        // Postgres vs SQLite placeholder handled by SeaORM's Statement builder
        txn.execute(Statement::from_sql_and_values(
            txn.get_database_backend(),
            sql,
            vec![tenant_id.into(), subject_id.into()],
        )).await?;
    }
    Ok(())
}

/// Normalizes constraints via `audience.rs` central helpers (already `pub(crate)`).
/// This is the second-level dedup: 4 services no longer reimplement `normalize_*`.
pub fn audience_constraints_needs_storage(constraints: &ForumAudienceConstraints) -> bool {
    !constraints.roles_any.is_empty()
        || !constraints.channel_members_any.is_empty()
        || !constraints.group_members_any.is_empty()
        || !constraints.allow_user_ids.is_empty()
        || !constraints.deny_user_ids.is_empty()
        || constraints.minimum_trust_level.is_some()
}
