use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::Utc;
use rustok_api::{normalize_locale_tag, Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_profiles::{ProfileService, ProfilesReader};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{
    forum_audience_mention, forum_quote, forum_relation_revision, forum_reply, forum_topic,
    forum_user_mention,
};
use crate::error::{ForumError, ForumResult};
use crate::mentions::{
    extract_forum_mention_candidates, resolve_forum_mentions, validate_forum_quote_references,
    ForumContentTarget, ForumContentTargetKind, ForumMentionAudience, ForumMentionPolicy,
    ForumQuoteReference, ForumResolvedMentions, ForumRevisionIdentity,
    FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
};

pub(crate) struct MentionRelationService {
    profiles: Arc<dyn ProfilesReader>,
}

pub(crate) struct PreparedMentionRelations {
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: String,
    projection_fingerprint: String,
    resolved: ForumResolvedMentions,
    quotes: Vec<ForumQuoteReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MentionRelationSyncResult {
    source: ForumRevisionIdentity,
    replayed: bool,
    added_user_ids: Vec<Uuid>,
    added_audiences: Vec<ForumMentionAudience>,
    mention_count: usize,
    quote_count: usize,
}

impl MentionRelationSyncResult {
    pub(crate) fn source(&self) -> &ForumRevisionIdentity {
        &self.source
    }

    pub(crate) fn replayed(&self) -> bool {
        self.replayed
    }

    pub(crate) fn added_user_ids(&self) -> &[Uuid] {
        &self.added_user_ids
    }

    pub(crate) fn added_audiences(&self) -> &[ForumMentionAudience] {
        &self.added_audiences
    }

    pub(crate) fn mention_count(&self) -> usize {
        self.mention_count
    }

    pub(crate) fn quote_count(&self) -> usize {
        self.quote_count
    }
}

impl MentionRelationService {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self::with_profiles(Arc::new(ProfileService::new(db)))
    }

    pub(crate) fn with_profiles(profiles: Arc<dyn ProfilesReader>) -> Self {
        Self { profiles }
    }

    pub(crate) async fn prepare(
        &self,
        tenant_id: Uuid,
        target: ForumContentTarget,
        locale: &str,
        body: &str,
        body_format: &str,
        security: &SecurityContext,
        quotes: impl IntoIterator<Item = ForumQuoteReference>,
    ) -> ForumResult<PreparedMentionRelations> {
        if tenant_id.is_nil() || target.id().is_nil() {
            return Err(ForumError::Validation(
                "Forum relation source requires non-nil tenant and target IDs".to_string(),
            ));
        }
        let locale = normalize_locale_tag(locale).ok_or_else(|| {
            ForumError::Validation("Forum relation source requires a valid locale".to_string())
        })?;
        let policy = ForumMentionPolicy {
            allow_moderator_audience: !matches!(
                security.get_scope(source_resource(target.kind()), Action::Moderate),
                PermissionScope::None
            ),
            ..ForumMentionPolicy::default()
        };
        let candidates = extract_forum_mention_candidates(body, body_format, &locale, policy)?;
        let resolved = resolve_forum_mentions(
            self.profiles.as_ref(),
            tenant_id,
            candidates,
            Some(&locale),
            Some(&locale),
        )
        .await?;
        let quotes = quotes.into_iter().collect::<BTreeSet<_>>();
        if quotes.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
            return Err(ForumError::Validation(format!(
                "Forum revision exceeds the {FORUM_MAX_QUOTE_REFERENCES_PER_REVISION}-quote limit"
            )));
        }
        let quotes = quotes.into_iter().collect::<Vec<_>>();
        let projection_fingerprint = projection_fingerprint(
            body_format,
            body,
            resolved.users(),
            resolved.audiences(),
            &quotes,
        );

        Ok(PreparedMentionRelations {
            tenant_id,
            target,
            locale,
            projection_fingerprint,
            resolved,
            quotes,
        })
    }

    pub(crate) async fn persist_in_tx(
        &self,
        txn: &DatabaseTransaction,
        prepared: PreparedMentionRelations,
    ) -> ForumResult<MentionRelationSyncResult> {
        lock_source_in_tx(txn, prepared.tenant_id, prepared.target).await?;
        let latest = latest_revision_in_tx(
            txn,
            prepared.tenant_id,
            prepared.target,
            &prepared.locale,
        )
        .await?;
        let current_snapshot = ProjectionSnapshot::from_prepared(&prepared);

        if let Some(latest) = latest.as_ref() {
            if latest.projection_fingerprint == prepared.projection_fingerprint {
                let persisted = load_snapshot_in_tx(txn, prepared.tenant_id, latest.revision_id).await?;
                if persisted != current_snapshot {
                    return Err(ForumError::Validation(
                        "Forum relation replay fingerprint does not match persisted targets"
                            .to_string(),
                    ));
                }
                return Ok(MentionRelationSyncResult {
                    source: ForumRevisionIdentity::new(
                        prepared.tenant_id,
                        prepared.target,
                        latest.revision_id,
                        prepared.locale,
                    )?,
                    replayed: true,
                    added_user_ids: Vec::new(),
                    added_audiences: Vec::new(),
                    mention_count: current_snapshot.users.len()
                        + current_snapshot.audiences.len(),
                    quote_count: current_snapshot.quotes.len(),
                });
            }
        }

        let previous_snapshot = if let Some(latest) = latest.as_ref() {
            load_snapshot_in_tx(txn, prepared.tenant_id, latest.revision_id).await?
        } else {
            ProjectionSnapshot::default()
        };

        let revision = forum_relation_revision::ActiveModel {
            revision_id: NotSet,
            tenant_id: Set(prepared.tenant_id),
            target_kind: Set(target_kind_value(prepared.target.kind()).to_string()),
            target_id: Set(prepared.target.id()),
            locale: Set(prepared.locale.clone()),
            projection_fingerprint: Set(prepared.projection_fingerprint),
            created_at: Set(Utc::now().into()),
        }
        .insert(txn)
        .await?;
        let source = ForumRevisionIdentity::new(
            prepared.tenant_id,
            prepared.target,
            revision.revision_id,
            prepared.locale.clone(),
        )?;
        let quotes = validate_forum_quote_references(&source, prepared.quotes)?;
        validate_quote_targets_in_tx(txn, prepared.tenant_id, &quotes).await?;

        let now = Utc::now();
        for mention in prepared.resolved.users() {
            forum_user_mention::ActiveModel {
                tenant_id: Set(prepared.tenant_id),
                source_kind: Set(target_kind_value(prepared.target.kind()).to_string()),
                source_id: Set(prepared.target.id()),
                source_locale: Set(prepared.locale.clone()),
                source_revision_id: Set(revision.revision_id),
                mentioned_user_id: Set(mention.user_id()),
                handle_snapshot: Set(mention.handle().to_string()),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }
        for audience in prepared.resolved.audiences() {
            forum_audience_mention::ActiveModel {
                tenant_id: Set(prepared.tenant_id),
                source_kind: Set(target_kind_value(prepared.target.kind()).to_string()),
                source_id: Set(prepared.target.id()),
                source_locale: Set(prepared.locale.clone()),
                source_revision_id: Set(revision.revision_id),
                audience: Set(audience_value(*audience).to_string()),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }
        for quote in &quotes {
            forum_quote::ActiveModel {
                tenant_id: Set(prepared.tenant_id),
                source_kind: Set(target_kind_value(prepared.target.kind()).to_string()),
                source_id: Set(prepared.target.id()),
                source_locale: Set(prepared.locale.clone()),
                source_revision_id: Set(revision.revision_id),
                quoted_kind: Set(target_kind_value(quote.target().kind()).to_string()),
                quoted_id: Set(quote.target().id()),
                quoted_revision_id: Set(quote.revision_id()),
                created_at: Set(now.into()),
            }
            .insert(txn)
            .await?;
        }

        let current_user_ids = current_snapshot
            .users
            .iter()
            .map(|(user_id, _)| *user_id)
            .collect::<BTreeSet<_>>();
        let previous_user_ids = previous_snapshot
            .users
            .iter()
            .map(|(user_id, _)| *user_id)
            .collect::<BTreeSet<_>>();
        let added_user_ids = current_user_ids
            .difference(&previous_user_ids)
            .copied()
            .collect();
        let added_audiences = current_snapshot
            .audiences
            .difference(&previous_snapshot.audiences)
            .copied()
            .collect();

        Ok(MentionRelationSyncResult {
            source,
            replayed: false,
            added_user_ids,
            added_audiences,
            mention_count: current_snapshot.users.len() + current_snapshot.audiences.len(),
            quote_count: current_snapshot.quotes.len(),
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ProjectionSnapshot {
    users: BTreeSet<(Uuid, String)>,
    audiences: BTreeSet<ForumMentionAudience>,
    quotes: BTreeSet<(ForumContentTarget, i64)>,
}

impl ProjectionSnapshot {
    fn from_prepared(prepared: &PreparedMentionRelations) -> Self {
        Self {
            users: prepared
                .resolved
                .users()
                .iter()
                .map(|mention| (mention.user_id(), mention.handle().to_string()))
                .collect(),
            audiences: prepared.resolved.audiences().iter().copied().collect(),
            quotes: prepared
                .quotes
                .iter()
                .map(|quote| (quote.target(), quote.revision_id()))
                .collect(),
        }
    }
}

async fn latest_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target: ForumContentTarget,
    locale: &str,
) -> ForumResult<Option<forum_relation_revision::Model>> {
    Ok(forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(
            forum_relation_revision::Column::TargetKind
                .eq(target_kind_value(target.kind())),
        )
        .filter(forum_relation_revision::Column::TargetId.eq(target.id()))
        .filter(forum_relation_revision::Column::Locale.eq(locale))
        .order_by_desc(forum_relation_revision::Column::RevisionId)
        .one(txn)
        .await?)
}

async fn load_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    revision_id: i64,
) -> ForumResult<ProjectionSnapshot> {
    let users = forum_user_mention::Entity::find()
        .filter(forum_user_mention::Column::TenantId.eq(tenant_id))
        .filter(forum_user_mention::Column::SourceRevisionId.eq(revision_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|row| (row.mentioned_user_id, row.handle_snapshot))
        .collect();
    let audiences = forum_audience_mention::Entity::find()
        .filter(forum_audience_mention::Column::TenantId.eq(tenant_id))
        .filter(forum_audience_mention::Column::SourceRevisionId.eq(revision_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|row| parse_audience(&row.audience))
        .collect::<ForumResult<BTreeSet<_>>>()?;
    let quotes = forum_quote::Entity::find()
        .filter(forum_quote::Column::TenantId.eq(tenant_id))
        .filter(forum_quote::Column::SourceRevisionId.eq(revision_id))
        .all(txn)
        .await?
        .into_iter()
        .map(|row| {
            Ok((
                ForumContentTarget::new(parse_target_kind(&row.quoted_kind)?, row.quoted_id)?,
                row.quoted_revision_id,
            ))
        })
        .collect::<ForumResult<BTreeSet<_>>>()?;
    Ok(ProjectionSnapshot {
        users,
        audiences,
        quotes,
    })
}

async fn validate_quote_targets_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    quotes: &[ForumQuoteReference],
) -> ForumResult<()> {
    for quote in quotes {
        let target = quote.target();
        let exists = forum_relation_revision::Entity::find_by_id(quote.revision_id())
            .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
            .filter(
                forum_relation_revision::Column::TargetKind
                    .eq(target_kind_value(target.kind())),
            )
            .filter(forum_relation_revision::Column::TargetId.eq(target.id()))
            .one(txn)
            .await?
            .is_some();
        if !exists {
            return Err(ForumError::quote_target_unavailable());
        }
    }
    Ok(())
}

async fn lock_source_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target: ForumContentTarget,
) -> ForumResult<()> {
    match target.kind() {
        ForumContentTargetKind::Topic => {
            let query = || {
                forum_topic::Entity::find_by_id(target.id())
                    .filter(forum_topic::Column::TenantId.eq(tenant_id))
            };
            let found = match txn.get_database_backend() {
                DbBackend::Sqlite => query().one(txn).await?,
                DbBackend::Postgres | DbBackend::MySql => {
                    query().lock_exclusive().one(txn).await?
                }
            };
            if found.is_none() {
                return Err(ForumError::TopicNotFound(target.id()));
            }
        }
        ForumContentTargetKind::Reply => {
            let query = || {
                forum_reply::Entity::find_by_id(target.id())
                    .filter(forum_reply::Column::TenantId.eq(tenant_id))
            };
            let found = match txn.get_database_backend() {
                DbBackend::Sqlite => query().one(txn).await?,
                DbBackend::Postgres | DbBackend::MySql => {
                    query().lock_exclusive().one(txn).await?
                }
            };
            if found.is_none() {
                return Err(ForumError::ReplyNotFound(target.id()));
            }
        }
    }
    Ok(())
}

fn projection_fingerprint(
    body_format: &str,
    body: &str,
    users: &[crate::mentions::ResolvedForumMention],
    audiences: &[ForumMentionAudience],
    quotes: &[ForumQuoteReference],
) -> String {
    let mut digest = Sha256::new();
    update_digest(&mut digest, body_format.as_bytes());
    update_digest(&mut digest, body.as_bytes());
    for mention in users {
        update_digest(&mut digest, mention.user_id().as_bytes());
        update_digest(&mut digest, mention.handle().as_bytes());
    }
    for audience in audiences {
        update_digest(&mut digest, audience_value(*audience).as_bytes());
    }
    for quote in quotes {
        update_digest(
            &mut digest,
            target_kind_value(quote.target().kind()).as_bytes(),
        );
        update_digest(&mut digest, quote.target().id().as_bytes());
        update_digest(&mut digest, &quote.revision_id().to_be_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn update_digest(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn source_resource(kind: ForumContentTargetKind) -> Resource {
    match kind {
        ForumContentTargetKind::Topic => Resource::ForumTopics,
        ForumContentTargetKind::Reply => Resource::ForumReplies,
    }
}

fn target_kind_value(kind: ForumContentTargetKind) -> &'static str {
    match kind {
        ForumContentTargetKind::Topic => "topic",
        ForumContentTargetKind::Reply => "reply",
    }
}

fn parse_target_kind(value: &str) -> ForumResult<ForumContentTargetKind> {
    match value {
        "topic" => Ok(ForumContentTargetKind::Topic),
        "reply" => Ok(ForumContentTargetKind::Reply),
        _ => Err(ForumError::Validation(
            "Persisted Forum relation target kind is invalid".to_string(),
        )),
    }
}

fn audience_value(audience: ForumMentionAudience) -> &'static str {
    match audience {
        ForumMentionAudience::Moderators => "moderators",
    }
}

fn parse_audience(value: &str) -> ForumResult<ForumMentionAudience> {
    match value {
        "moderators" => Ok(ForumMentionAudience::Moderators),
        _ => Err(ForumError::Validation(
            "Persisted Forum mention audience is invalid".to_string(),
        )),
    }
}
