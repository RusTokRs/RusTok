use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement};
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
    txn.execute_raw(Statement::from_sql_and_values(
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
    txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), sql, vals)).await.map(|_| ()).map_err(ForumError::from)
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
    txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), sql, vals)).await.map(|_| ()).map_err(ForumError::from)
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
    txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), sql, vals)).await.map(|_| ()).map_err(ForumError::from)
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
        txn.execute_raw(Statement::from_sql_and_values(
            txn.get_database_backend(),
            sql,
            vec![tenant_id.into(), subject_id.into()],
        )).await?;
    }
    Ok(())
}

/// Generic raw loader for the 6× `load_local_layers` copy-paste.
/// Each subject (category/topic) has at most one policy row + 0..N role/channel/group/user rows.
/// Returns `HashMap<subject_id, constraints>` with normalized constraints.
/// Stored via raw `query_all` so no EntityTrait bound is needed — table name drives the read.
pub async fn load_audience_layers_raw<C>(
    db: &C,
    tenant_id: Uuid,
    subject_ids: &[Uuid],
    table_prefix: &str,
    subject_column: &str,
) -> ForumResult<std::collections::HashMap<Uuid, ForumAudienceConstraints>>
where
    C: ConnectionTrait,
{
    use sea_orm::Statement;
    if subject_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut layers = std::collections::HashMap::new();

    // 1) policies
    {
        let table = format!("{table_prefix}_policies");
        let mut sql = format!("SELECT {subject_column}, minimum_trust_level FROM {table} WHERE tenant_id = $1 AND {subject_column} IN (");
        for (i, _) in subject_ids.iter().enumerate() {
            if i > 0 { sql.push_str(", "); }
            sql.push_str(&format!("${}", i + 2));
        }
        sql.push(')');
        let mut vals: Vec<sea_orm::Value> = vec![tenant_id.into()];
        vals.extend(subject_ids.iter().map(|id| (*id).into()));
        let rows = db.query_all_raw(Statement::from_sql_and_values(db.get_database_backend(), sql, vals)).await?;
        for row in rows {
            let sid: Uuid = row.try_get_by_index(0)?;
            let trust: Option<i16> = row.try_get_by_index(1)?;
            let level = trust.map(|v| u8::try_from(v).map_err(|_| crate::error::ForumError::Validation("Forum audience storage contains an invalid trust level".to_string()))).transpose()?;
            layers.insert(sid, ForumAudienceConstraints { minimum_trust_level: level, ..Default::default() });
        }
    }

    // 2) roles
    {
        let table = format!("{table_prefix}_roles");
        let mut sql = format!("SELECT {subject_column}, role FROM {table} WHERE tenant_id = $1 AND {subject_column} IN (");
        for (i, _) in subject_ids.iter().enumerate() { if i > 0 { sql.push_str(", "); } sql.push_str(&format!("${}", i+2)); }
        sql.push(')');
        let mut vals: Vec<sea_orm::Value> = vec![tenant_id.into()];
        vals.extend(subject_ids.iter().map(|id| (*id).into()));
        let rows = db.query_all_raw(Statement::from_sql_and_values(db.get_database_backend(), sql, vals)).await?;
        if rows.len() > subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_ROLES {
            return Err(crate::error::ForumError::Validation(format!("Forum audience storage exceeds the bounded role relations limit of {}", subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_ROLES)));
        }
        for row in rows {
            let sid: Uuid = row.try_get_by_index(0)?;
            let val: String = row.try_get_by_index(1)?;
            let entry = layers.get_mut(&sid).ok_or_else(|| crate::error::ForumError::Validation("Forum audience relation is missing its local policy layer".to_string()))?;
            let role = val.parse().map_err(|_| {
                ForumError::Validation("Forum audience storage contains an invalid role".to_string())
            })?;
            entry.roles_any.push(role);
        }
    }

    // 3) channels
    {
        let table = format!("{table_prefix}_channels");
        let mut sql = format!("SELECT {subject_column}, channel_slug FROM {table} WHERE tenant_id = $1 AND {subject_column} IN (");
        for (i, _) in subject_ids.iter().enumerate() { if i > 0 { sql.push_str(", "); } sql.push_str(&format!("${}", i+2)); }
        sql.push(')');
        let mut vals: Vec<sea_orm::Value> = vec![tenant_id.into()];
        vals.extend(subject_ids.iter().map(|id| (*id).into()));
        let rows = db.query_all_raw(Statement::from_sql_and_values(db.get_database_backend(), sql, vals)).await?;
        if rows.len() > subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_CHANNELS {
            return Err(crate::error::ForumError::Validation(format!("Forum audience storage exceeds the bounded channel relations limit of {}", subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_CHANNELS)));
        }
        for row in rows {
            let sid: Uuid = row.try_get_by_index(0)?;
            let val: String = row.try_get_by_index(1)?;
            let entry = layers.get_mut(&sid).ok_or_else(|| crate::error::ForumError::Validation("Forum audience relation is missing its local policy layer".to_string()))?;
            entry.channel_members_any.push(val);
        }
    }

    // 4) groups
    {
        let table = format!("{table_prefix}_groups");
        let mut sql = format!("SELECT {subject_column}, group_id FROM {table} WHERE tenant_id = $1 AND {subject_column} IN (");
        for (i, _) in subject_ids.iter().enumerate() { if i > 0 { sql.push_str(", "); } sql.push_str(&format!("${}", i+2)); }
        sql.push(')');
        let mut vals: Vec<sea_orm::Value> = vec![tenant_id.into()];
        vals.extend(subject_ids.iter().map(|id| (*id).into()));
        let rows = db.query_all_raw(Statement::from_sql_and_values(db.get_database_backend(), sql, vals)).await?;
        if rows.len() > subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_GROUPS {
            return Err(crate::error::ForumError::Validation(format!("Forum audience storage exceeds the bounded group relations limit of {}", subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_GROUPS)));
        }
        for row in rows {
            let sid: Uuid = row.try_get_by_index(0)?;
            let val: Uuid = row.try_get_by_index(1)?;
            let entry = layers.get_mut(&sid).ok_or_else(|| crate::error::ForumError::Validation("Forum audience relation is missing its local policy layer".to_string()))?;
            entry.group_members_any.push(val);
        }
    }

    // 5) users: (subject_id, user_id, effect)
    {
        let table = format!("{table_prefix}_users");
        let mut sql = format!("SELECT {subject_column}, user_id, effect FROM {table} WHERE tenant_id = $1 AND {subject_column} IN (");
        for (i, _) in subject_ids.iter().enumerate() { if i > 0 { sql.push_str(", "); } sql.push_str(&format!("${}", i+2)); }
        sql.push(')');
        let mut vals: Vec<sea_orm::Value> = vec![tenant_id.into()];
        vals.extend(subject_ids.iter().map(|id| (*id).into()));
        let rows = db.query_all_raw(Statement::from_sql_and_values(db.get_database_backend(), sql, vals)).await?;
        if rows.len() > subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2 {
            return Err(crate::error::ForumError::Validation(format!("Forum audience storage exceeds the bounded explicit user relations limit of {}", subject_ids.len() * crate::audience::MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2)));
        }
        for row in rows {
            let sid: Uuid = row.try_get_by_index(0)?;
            let uid: Uuid = row.try_get_by_index(1)?;
            let effect: String = row.try_get_by_index(2)?;
            let entry = layers.get_mut(&sid).ok_or_else(|| crate::error::ForumError::Validation("Forum audience relation is missing its local policy layer".to_string()))?;
            match effect.as_str() {
                "allow" => entry.allow_user_ids.push(uid),
                "deny" => entry.deny_user_ids.push(uid),
                _ => {
                    return Err(ForumError::Validation(
                        "Forum audience storage contains an invalid user effect".to_string(),
                    ));
                }
            }
        }
    }

    for constraints in layers.values_mut() {
        *constraints = constraints.clone().normalize()?;
        if constraints.roles_any.is_empty() && constraints.minimum_trust_level.is_none() && constraints.channel_members_any.is_empty() && constraints.group_members_any.is_empty() && constraints.allow_user_ids.is_empty() && constraints.deny_user_ids.is_empty() {
            return Err(crate::error::ForumError::Validation("Forum audience storage contains an empty local layer".to_string()));
        }
    }
    Ok(layers)
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
