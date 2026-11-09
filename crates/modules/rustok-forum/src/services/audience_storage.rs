use std::collections::HashMap;

use crate::audience::{
    ForumAudienceConstraints, MAX_FORUM_AUDIENCE_CHANNELS, MAX_FORUM_AUDIENCE_EXPLICIT_USERS,
    MAX_FORUM_AUDIENCE_GROUPS, MAX_FORUM_AUDIENCE_ROLES,
};
use crate::error::{ForumError, ForumResult};
use sea_orm::{ConnectionTrait, DatabaseTransaction, QueryResult, Statement};
use uuid::Uuid;

/// Ideal helper: one place for the 4× `insert_roles/channels/groups/users` copy-paste
/// across `category_audience*`, `category_moderation_audience`, `topic_audience*`.
/// Tables are `{prefix}_roles|_channels|_groups|_users` with `(tenant_id, subject_id)` PK.
/// `subject_column` is `category_id` or `topic_id` depending on scope — passed by caller
/// so the generic does not need a full EntityTrait bound.
pub(super) async fn insert_roles_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    roles: &[String],
) -> ForumResult<()> {
    if roles.is_empty() {
        return Ok(());
    }
    let mut sql = format!("INSERT INTO {table} (tenant_id, {subject_column}, role) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, role) in roles.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!(
            "(${}, ${}, ${})",
            vals.len() + 1,
            vals.len() + 2,
            vals.len() + 3
        ));
        vals.push(tenant_id.into());
        vals.push(subject_id.into());
        vals.push(role.clone().into());
    }
    txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        sql,
        vals,
    ))
    .await
    .map(|_| ())
    .map_err(ForumError::from)
}

pub(super) async fn insert_channels_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    channels: &[String],
) -> ForumResult<()> {
    if channels.is_empty() {
        return Ok(());
    }
    let mut sql =
        format!("INSERT INTO {table} (tenant_id, {subject_column}, channel_slug) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, ch) in channels.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!(
            "(${}, ${}, ${})",
            vals.len() + 1,
            vals.len() + 2,
            vals.len() + 3
        ));
        vals.push(tenant_id.into());
        vals.push(subject_id.into());
        vals.push(ch.clone().into());
    }
    txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        sql,
        vals,
    ))
    .await
    .map(|_| ())
    .map_err(ForumError::from)
}

pub(super) async fn insert_groups_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    groups: &[Uuid],
) -> ForumResult<()> {
    if groups.is_empty() {
        return Ok(());
    }
    let mut sql = format!("INSERT INTO {table} (tenant_id, {subject_column}, group_id) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!(
            "(${}, ${}, ${})",
            vals.len() + 1,
            vals.len() + 2,
            vals.len() + 3
        ));
        vals.push(tenant_id.into());
        vals.push(subject_id.into());
        vals.push((*g).into());
    }
    txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        sql,
        vals,
    ))
    .await
    .map(|_| ())
    .map_err(ForumError::from)
}

pub(super) async fn insert_users_raw(
    txn: &DatabaseTransaction,
    table: &str,
    tenant_id: Uuid,
    subject_id: Uuid,
    subject_column: &str,
    user_ids: &[Uuid],
    effect: &str,
) -> ForumResult<()> {
    if user_ids.is_empty() {
        return Ok(());
    }
    let mut sql =
        format!("INSERT INTO {table} (tenant_id, {subject_column}, user_id, effect) VALUES ");
    let mut vals: Vec<sea_orm::Value> = Vec::new();
    for (i, uid) in user_ids.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!(
            "(${}, ${}, ${}, ${})",
            vals.len() + 1,
            vals.len() + 2,
            vals.len() + 3,
            vals.len() + 4
        ));
        vals.push(tenant_id.into());
        vals.push(subject_id.into());
        vals.push((*uid).into());
        vals.push(effect.to_string().into());
    }
    txn.execute_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        sql,
        vals,
    ))
    .await
    .map(|_| ())
    .map_err(ForumError::from)
}

/// Reads normalized constraints only from the Forum-owned tables of one audience scope.
pub(super) async fn load_audience_layers_raw<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    subject_ids: &[Uuid],
    table_prefix: &str,
    subject_column: &str,
) -> ForumResult<HashMap<Uuid, ForumAudienceConstraints>> {
    if subject_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut layers = HashMap::new();
    for row in load_rows(
        db,
        tenant_id,
        subject_ids,
        subject_column,
        &format!("{table_prefix}_policies"),
        "minimum_trust_level",
        1,
    )
    .await?
    {
        let subject_id = row.try_get_by_index(0)?;
        let trust: Option<i16> = row.try_get_by_index(1)?;
        let minimum_trust_level = trust.map(u8::try_from).transpose().map_err(|_| {
            ForumError::Validation("Forum audience storage contains an invalid trust level".into())
        })?;
        let previous = layers.insert(
            subject_id,
            ForumAudienceConstraints {
                minimum_trust_level,
                ..Default::default()
            },
        );
        if previous.is_some() {
            return Err(ForumError::Validation(
                "Forum audience contains duplicate local policies".into(),
            ));
        }
    }
    for row in load_rows(
        db,
        tenant_id,
        subject_ids,
        subject_column,
        &format!("{table_prefix}_roles"),
        "role",
        MAX_FORUM_AUDIENCE_ROLES,
    )
    .await?
    {
        let role: String = row.try_get_by_index(1)?;
        let role = role.parse().map_err(|_| {
            ForumError::Validation("Forum audience storage contains an invalid role".into())
        })?;
        local_layer(&mut layers, &row)?.roles_any.push(role);
    }
    for row in load_rows(
        db,
        tenant_id,
        subject_ids,
        subject_column,
        &format!("{table_prefix}_channels"),
        "channel_slug",
        MAX_FORUM_AUDIENCE_CHANNELS,
    )
    .await?
    {
        let channel = row.try_get_by_index(1)?;
        local_layer(&mut layers, &row)?
            .channel_members_any
            .push(channel);
    }
    for row in load_rows(
        db,
        tenant_id,
        subject_ids,
        subject_column,
        &format!("{table_prefix}_groups"),
        "group_id",
        MAX_FORUM_AUDIENCE_GROUPS,
    )
    .await?
    {
        let group = row.try_get_by_index(1)?;
        local_layer(&mut layers, &row)?
            .group_members_any
            .push(group);
    }
    for row in load_rows(
        db,
        tenant_id,
        subject_ids,
        subject_column,
        &format!("{table_prefix}_users"),
        "user_id, effect",
        MAX_FORUM_AUDIENCE_EXPLICIT_USERS * 2,
    )
    .await?
    {
        let user_id = row.try_get_by_index(1)?;
        let effect: String = row.try_get_by_index(2)?;
        let layer = local_layer(&mut layers, &row)?;
        match effect.as_str() {
            "allow" => layer.allow_user_ids.push(user_id),
            "deny" => layer.deny_user_ids.push(user_id),
            _ => {
                return Err(ForumError::Validation(
                    "Forum audience storage contains an invalid user effect".into(),
                ));
            }
        }
    }

    layers
        .into_iter()
        .map(|(id, constraints)| {
            let constraints = constraints.normalize()?;
            if !constraints.has_positive_selectors() && constraints.deny_user_ids.is_empty() {
                return Err(ForumError::Validation(
                    "Forum audience storage contains an empty local layer".into(),
                ));
            }
            Ok((id, constraints))
        })
        .collect()
}

fn local_layer<'a>(
    layers: &'a mut HashMap<Uuid, ForumAudienceConstraints>,
    row: &QueryResult,
) -> ForumResult<&'a mut ForumAudienceConstraints> {
    let subject_id = row.try_get_by_index::<Uuid>(0)?;
    layers.get_mut(&subject_id).ok_or_else(|| {
        ForumError::Validation("Forum audience relation is missing its local policy layer".into())
    })
}

async fn load_rows<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    subject_ids: &[Uuid],
    subject_column: &str,
    table: &str,
    columns: &str,
    max_per_subject: usize,
) -> ForumResult<Vec<QueryResult>> {
    let fetch_limit = subject_ids
        .len()
        .checked_mul(max_per_subject)
        .and_then(|limit| limit.checked_add(1))
        .ok_or_else(|| {
            ForumError::Validation("Forum audience query exceeds the row bound".into())
        })?;
    let placeholders = subject_ids
        .iter()
        .enumerate()
        .map(|(index, _)| format!("${}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {subject_column}, {columns} FROM {table}
         WHERE tenant_id = $1 AND {subject_column} IN ({placeholders}) LIMIT {fetch_limit}"
    );
    let mut values = Vec::with_capacity(subject_ids.len() + 1);
    values.push(sea_orm::Value::from(tenant_id));
    values.extend(subject_ids.iter().copied().map(sea_orm::Value::from));
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            values,
        ))
        .await?;
    if rows.len() == fetch_limit {
        return Err(ForumError::Validation(format!(
            "Forum audience rows in {table} exceed the bound"
        )));
    }
    Ok(rows)
}
