use chrono::Utc;
use sea_orm::{
    prelude::DateTimeWithTimeZone, ActiveModelTrait, ColumnTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, Set, TransactionTrait,
};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

use rustok_core::{Action, DomainEvent, PermissionScope, Resource, SecurityContext};
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    BodyInput, BodyResponse, CreateNodeInput, ListNodesFilter, NodeListItem, NodeResponse,
    NodeTranslationResponse, UpdateNodeInput,
};
use crate::entities::{body, node, node_translation};
use crate::error::{ContentError, ContentResult};

pub struct NodeService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl NodeService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub(crate) fn event_bus(&self) -> &TransactionalEventBus {
        &self.event_bus
    }

    fn kind_to_resource(kind: &str) -> ContentResult<Resource> {
        match kind {
            "post" | "article" | "custom" => Ok(Resource::Posts),
            "page" => Ok(Resource::Pages),
            "comment" => Ok(Resource::Comments),
            "blog_post" => Ok(Resource::BlogPosts),
            "forum_category" => Ok(Resource::ForumCategories),
            "forum_topic" => Ok(Resource::ForumTopics),
            "forum_reply" => Ok(Resource::ForumReplies),
            _ => Err(ContentError::Validation(format!(
                "Unsupported kind for RBAC mapping: {kind}"
            ))),
        }
    }

    /// Enforce permission scope and return whether access is allowed
    fn enforce_scope(
        &self,
        scope: PermissionScope,
        resource_author_id: Option<Uuid>,
        current_user_id: Option<Uuid>,
    ) -> ContentResult<()> {
        match scope {
            PermissionScope::All => Ok(()),
            PermissionScope::Own => {
                if resource_author_id == current_user_id {
                    Ok(())
                } else {
                    Err(ContentError::Forbidden(
                        "Permission denied: Not the author".into(),
                    ))
                }
            }
            PermissionScope::None => Err(ContentError::Forbidden("Permission denied".into())),
        }
    }

    /// Check if slug is unique within tenant and locale
    async fn ensure_slug_unique<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        locale: &str,
        slug: &str,
        exclude_node_id: Option<Uuid>,
    ) -> ContentResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let mut query = node_translation::Entity::find()
            .inner_join(node::Entity)
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node_translation::Column::Locale.eq(locale))
            .filter(node_translation::Column::Slug.eq(slug))
            .filter(node::Column::DeletedAt.is_null());

        if let Some(exclude_id) = exclude_node_id {
            query = query.filter(node::Column::Id.ne(exclude_id));
        }

        let existing = query.one(db).await?;

        if existing.is_some() {
            return Err(ContentError::duplicate_slug(slug, locale));
        }

        Ok(())
    }

    /// Check optimistic locking version
    fn check_version(&self, expected: Option<i32>, actual: i32) -> ContentResult<()> {
        if let Some(expected) = expected {
            if expected != actual {
                return Err(ContentError::concurrent_modification(expected, actual));
            }
        }
        Ok(())
    }

    #[instrument(skip(self, security, input), fields(tenant_id = %tenant_id, kind = %input.kind, user_id = ?security.user_id))]
    pub async fn create_node(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateNodeInput,
    ) -> ContentResult<NodeResponse> {
        info!("Creating node");
        let txn = self.db.begin().await?;
        let node_id = self
            .create_node_in_tx(&txn, tenant_id, security, input)
            .await?;
        txn.commit().await?;
        info!(node_id = %node_id, "Node created successfully");
        let response = self.get_node(tenant_id, node_id).await?;
        Ok(response)
    }

    /// Insert a node within an existing transaction. Does not begin or commit.
    /// Returns the new node id. Callers must commit the transaction themselves.
    pub async fn create_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        security: SecurityContext,
        mut input: CreateNodeInput,
    ) -> ContentResult<Uuid> {
        input
            .validate()
            .map_err(|e| ContentError::Validation(e.to_string()))?;

        let resource = Self::kind_to_resource(&input.kind)?;
        let scope = security.get_scope(resource, Action::Create);

        match scope {
            PermissionScope::All => {
                debug!("User has All scope for node creation");
            }
            PermissionScope::Own => {
                debug!("User has Own scope, setting author_id to user_id");
                input.author_id = security.user_id;
            }
            PermissionScope::None => {
                warn!("User lacks permission to create node");
                return Err(ContentError::Forbidden("Permission denied".into()));
            }
        }

        let now: DateTimeWithTimeZone = Utc::now().into();
        let node_id = rustok_core::generate_id();
        let status = input
            .status
            .unwrap_or(crate::entities::node::ContentStatus::Draft);
        let metadata = input.metadata;

        if input.translations.is_empty() {
            error!("Node creation failed: no translations provided");
            return Err(ContentError::Validation(
                "At least one translation is required".to_string(),
            ));
        }

        debug!(
            translations_count = input.translations.len(),
            bodies_count = input.bodies.len(),
            "Inserting node in transaction"
        );

        for translation in &input.translations {
            if let Some(ref slug) = translation.slug {
                self.ensure_slug_unique(txn, tenant_id, &translation.locale, slug, None)
                    .await?;
            }
        }

        node::ActiveModel {
            id: Set(node_id),
            tenant_id: Set(tenant_id),
            parent_id: Set(input.parent_id),
            author_id: Set(input.author_id),
            kind: Set(input.kind.clone()),
            category_id: Set(input.category_id),
            status: Set(status.clone()),
            position: Set(input.position.unwrap_or(0)),
            depth: Set(input.depth.unwrap_or(0)),
            reply_count: Set(input.reply_count.unwrap_or(0)),
            metadata: Set(metadata),
            created_at: Set(now),
            updated_at: Set(now),
            published_at: if status == crate::entities::node::ContentStatus::Published {
                Set(Some(now))
            } else {
                Set(None)
            },
            deleted_at: Set(None),
            version: Set(1),
        }
        .insert(txn)
        .await?;

        for translation in input.translations {
            let slug = resolve_slug(translation.slug, translation.title.as_ref(), &input.kind)?;

            if let Some(ref s) = slug {
                self.ensure_slug_unique(txn, tenant_id, &translation.locale, s, None)
                    .await?;
            }

            node_translation::ActiveModel {
                id: Set(rustok_core::generate_id()),
                node_id: Set(node_id),
                locale: Set(translation.locale.clone()),
                title: Set(translation.title),
                slug: Set(slug),
                excerpt: Set(translation.excerpt),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(txn)
            .await?;
        }

        for body_input in input.bodies {
            upsert_body(txn, node_id, body_input, now).await?;
        }

        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                security.user_id,
                DomainEvent::NodeCreated {
                    node_id,
                    kind: input.kind.clone(),
                    author_id: input.author_id,
                },
            )
            .await?;

        Ok(node_id)
    }

    #[instrument(skip(self, security, update), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn update_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
        update: UpdateNodeInput,
    ) -> ContentResult<NodeResponse> {
        info!("Updating node");
        let txn = self.db.begin().await?;
        let updated = self
            .update_node_in_tx(&txn, tenant_id, node_id, security, update)
            .await?;
        txn.commit().await?;
        let translations = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;
        let bodies = body::Entity::find()
            .filter(body::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;
        Ok(Self::to_response(updated, translations, bodies))
    }

    /// Update a node within an existing transaction. Does not begin or commit.
    /// Returns the updated node model. Callers must commit the transaction themselves.
    pub async fn update_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
        update: UpdateNodeInput,
    ) -> ContentResult<node::Model> {
        update
            .validate()
            .map_err(|e| ContentError::Validation(e.to_string()))?;

        let node_model = self.find_node(tenant_id, node_id).await?;

        if node_model.deleted_at.is_some() {
            return Err(ContentError::Validation(
                "Cannot update deleted node".to_string(),
            ));
        }

        self.check_version(update.expected_version, node_model.version)?;

        let resource = Self::kind_to_resource(&node_model.kind)?;
        let scope = security.get_scope(resource, Action::Update);
        self.enforce_scope(scope, node_model.author_id, security.user_id)?;

        if matches!(scope, PermissionScope::Own) && update.author_id.is_some() {
            return Err(ContentError::Forbidden(
                "Permission denied: cannot change author".into(),
            ));
        }

        let mut active: node::ActiveModel = node_model.clone().into();
        let now: DateTimeWithTimeZone = Utc::now().into();

        if let Some(parent_id) = update.parent_id {
            active.parent_id = Set(parent_id);
        }
        if let Some(author_id) = update.author_id {
            active.author_id = Set(author_id);
        }
        if let Some(category_id) = update.category_id {
            active.category_id = Set(category_id);
        }
        if let Some(status) = update.status.clone() {
            active.status = Set(status.clone());
            match status {
                crate::entities::node::ContentStatus::Published
                    if node_model.published_at.is_none() =>
                {
                    active.published_at = Set(Some(now));
                }
                crate::entities::node::ContentStatus::Draft => {
                    active.published_at = Set(None);
                }
                _ => {}
            }
        }
        if let Some(position) = update.position {
            active.position = Set(position);
        }
        if let Some(depth) = update.depth {
            active.depth = Set(depth);
        }
        if let Some(reply_count) = update.reply_count {
            active.reply_count = Set(reply_count);
        }
        if let Some(metadata) = update.metadata {
            active.metadata = Set(metadata);
        }

        active.updated_at = Set(now);
        active.version = Set(node_model.version + 1);

        if let Some(translations) = update.translations {
            for translation in &translations {
                if let Some(ref slug) = translation.slug {
                    self.ensure_slug_unique(
                        txn,
                        node_model.tenant_id,
                        &translation.locale,
                        slug,
                        Some(node_id),
                    )
                    .await?;
                }
            }

            node_translation::Entity::delete_many()
                .filter(node_translation::Column::NodeId.eq(node_id))
                .exec(txn)
                .await?;

            for translation in translations {
                let slug = resolve_slug(
                    translation.slug,
                    translation.title.as_ref(),
                    &node_model.kind,
                )?;

                if let Some(ref s) = slug {
                    self.ensure_slug_unique(
                        txn,
                        node_model.tenant_id,
                        &translation.locale,
                        s,
                        Some(node_id),
                    )
                    .await?;
                }

                node_translation::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    node_id: Set(node_id),
                    locale: Set(translation.locale),
                    title: Set(translation.title),
                    slug: Set(slug),
                    excerpt: Set(translation.excerpt),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?;
            }
        }

        if let Some(bodies) = update.bodies {
            body::Entity::delete_many()
                .filter(body::Column::NodeId.eq(node_id))
                .exec(txn)
                .await?;

            for body_input in bodies {
                upsert_body(txn, node_id, body_input, now).await?;
            }
        }

        let updated = active.update(txn).await?;

        self.event_bus
            .publish_in_tx(
                txn,
                updated.tenant_id,
                security.user_id,
                DomainEvent::NodeUpdated {
                    node_id: updated.id,
                    kind: updated.kind.clone(),
                },
            )
            .await?;

        Ok(updated)
    }

    /// Common method for status transitions (publish/unpublish/archive)
    async fn transition_status(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
        new_status: node::ContentStatus,
        events: Vec<DomainEvent>,
    ) -> ContentResult<NodeResponse> {
        let txn = self.db.begin().await?;
        let updated = self
            .transition_status_in_tx(&txn, tenant_id, node_id, security, new_status, events)
            .await?;
        txn.commit().await?;

        let translations = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;
        let bodies = body::Entity::find()
            .filter(body::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;

        Ok(Self::to_response(updated, translations, bodies))
    }

    /// Status transition within an existing transaction. Does not begin or commit.
    pub async fn transition_status_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
        new_status: node::ContentStatus,
        events: Vec<DomainEvent>,
    ) -> ContentResult<node::Model> {
        let node_model = self.find_node(tenant_id, node_id).await?;

        if node_model.deleted_at.is_some() {
            return Err(ContentError::Validation(
                "Cannot change status of deleted node".to_string(),
            ));
        }

        let resource = Self::kind_to_resource(&node_model.kind)?;
        let scope = security.get_scope(resource, Action::Update);
        self.enforce_scope(scope, node_model.author_id, security.user_id)?;

        let now: DateTimeWithTimeZone = Utc::now().into();
        let mut active: node::ActiveModel = node_model.clone().into();

        active.status = Set(new_status.clone());
        active.updated_at = Set(now);
        active.version = Set(node_model.version + 1);

        match new_status {
            node::ContentStatus::Published => {
                active.published_at = Set(Some(now));
            }
            node::ContentStatus::Draft | node::ContentStatus::Archived => {
                active.published_at = Set(None);
            }
        }

        let updated = active.update(txn).await?;

        for event in events {
            self.event_bus
                .publish_in_tx(txn, updated.tenant_id, security.user_id, event)
                .await?;
        }

        Ok(updated)
    }

    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn publish_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<NodeResponse> {
        info!("Publishing node");
        let node_model = self.find_node(tenant_id, node_id).await?;
        let kind = node_model.kind.clone();

        self.transition_status(
            tenant_id,
            node_id,
            security,
            node::ContentStatus::Published,
            vec![
                DomainEvent::NodeUpdated {
                    node_id,
                    kind: kind.clone(),
                },
                DomainEvent::NodePublished { node_id, kind },
            ],
        )
        .await
    }

    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn unpublish_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<NodeResponse> {
        info!("Unpublishing node");
        let node_model = self.find_node(tenant_id, node_id).await?;
        let kind = node_model.kind.clone();

        self.transition_status(
            tenant_id,
            node_id,
            security,
            node::ContentStatus::Draft,
            vec![
                DomainEvent::NodeUpdated {
                    node_id,
                    kind: kind.clone(),
                },
                DomainEvent::NodeUnpublished { node_id, kind },
            ],
        )
        .await
    }

    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn archive_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<NodeResponse> {
        info!("Archiving node");
        let node_model = self.find_node(tenant_id, node_id).await?;
        let kind = node_model.kind.clone();

        self.transition_status(
            tenant_id,
            node_id,
            security,
            node::ContentStatus::Archived,
            vec![DomainEvent::NodeUpdated { node_id, kind }],
        )
        .await
    }

    /// Publish a node within an existing transaction. Does not begin or commit.
    pub async fn publish_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<node::Model> {
        let kind = self.find_node(tenant_id, node_id).await?.kind;
        self.transition_status_in_tx(
            txn,
            tenant_id,
            node_id,
            security,
            node::ContentStatus::Published,
            vec![
                DomainEvent::NodeUpdated {
                    node_id,
                    kind: kind.clone(),
                },
                DomainEvent::NodePublished { node_id, kind },
            ],
        )
        .await
    }

    /// Unpublish a node within an existing transaction. Does not begin or commit.
    pub async fn unpublish_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<node::Model> {
        let kind = self.find_node(tenant_id, node_id).await?.kind;
        self.transition_status_in_tx(
            txn,
            tenant_id,
            node_id,
            security,
            node::ContentStatus::Draft,
            vec![
                DomainEvent::NodeUpdated {
                    node_id,
                    kind: kind.clone(),
                },
                DomainEvent::NodeUnpublished { node_id, kind },
            ],
        )
        .await
    }

    /// Archive a node within an existing transaction. Does not begin or commit.
    pub async fn archive_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<node::Model> {
        let kind = self.find_node(tenant_id, node_id).await?.kind;
        self.transition_status_in_tx(
            txn,
            tenant_id,
            node_id,
            security,
            node::ContentStatus::Archived,
            vec![DomainEvent::NodeUpdated { node_id, kind }],
        )
        .await
    }

    /// Soft delete a node (can be restored)
    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn delete_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<()> {
        info!("Soft-deleting node");
        let txn = self.db.begin().await?;
        self.delete_node_in_tx(&txn, tenant_id, node_id, security)
            .await?;
        txn.commit().await?;
        info!(node_id = %node_id, "Node soft-deleted successfully");
        Ok(())
    }

    /// Soft-delete a node within an existing transaction. Does not begin or commit.
    pub async fn delete_node_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<()> {
        let node_model = self.find_node(tenant_id, node_id).await?;

        let resource = Self::kind_to_resource(&node_model.kind)?;
        let scope = security.get_scope(resource, Action::Delete);
        self.enforce_scope(scope, node_model.author_id, security.user_id)?;

        let now: DateTimeWithTimeZone = Utc::now().into();
        let mut active: node::ActiveModel = node_model.clone().into();
        active.deleted_at = Set(Some(now));
        active.updated_at = Set(now);
        active.version = Set(node_model.version + 1);
        active.update(txn).await?;

        self.event_bus
            .publish_in_tx(
                txn,
                node_model.tenant_id,
                security.user_id,
                DomainEvent::NodeDeleted {
                    node_id: node_model.id,
                    kind: node_model.kind,
                },
            )
            .await?;

        Ok(())
    }

    /// Restore a soft-deleted node
    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn restore_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<NodeResponse> {
        info!("Restoring node");
        let node_model = self.find_node_with_deleted(tenant_id, node_id).await?;

        if node_model.deleted_at.is_none() {
            return Err(ContentError::Validation("Node is not deleted".to_string()));
        }

        // Scope Enforcement
        let resource = Self::kind_to_resource(&node_model.kind)?;
        let scope = security.get_scope(resource, Action::Update);
        self.enforce_scope(scope, node_model.author_id, security.user_id)?;

        let now = Utc::now().into();
        let txn = self.db.begin().await?;

        let mut active: node::ActiveModel = node_model.clone().into();
        active.deleted_at = Set(None);
        active.updated_at = Set(now);
        active.version = Set(node_model.version + 1);
        let updated = active.update(&txn).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                updated.tenant_id,
                security.user_id,
                DomainEvent::NodeUpdated {
                    node_id: updated.id,
                    kind: updated.kind.clone(),
                },
            )
            .await?;

        txn.commit().await?;

        let translations = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;
        let bodies = body::Entity::find()
            .filter(body::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;

        info!(node_id = %node_id, "Node restored successfully");
        Ok(Self::to_response(updated, translations, bodies))
    }

    /// Hard delete - permanently removes node (admin only)
    #[instrument(skip(self, security), fields(tenant_id = %tenant_id, node_id = %node_id, user_id = ?security.user_id))]
    pub async fn hard_delete_node(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        security: SecurityContext,
    ) -> ContentResult<()> {
        info!("Hard-deleting node");
        let node_model = self.find_node_with_deleted(tenant_id, node_id).await?;

        // Only admins can hard delete
        let resource = Self::kind_to_resource(&node_model.kind)?;
        let scope = security.get_scope(resource, Action::Delete);
        if !matches!(scope, PermissionScope::All) {
            return Err(ContentError::Forbidden(
                "Hard delete requires admin privileges".into(),
            ));
        }

        let txn = self.db.begin().await?;

        // Delete related records first
        body::Entity::delete_many()
            .filter(body::Column::NodeId.eq(node_id))
            .exec(&txn)
            .await?;
        node_translation::Entity::delete_many()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .exec(&txn)
            .await?;
        node::Entity::delete_by_id(node_id).exec(&txn).await?;

        txn.commit().await?;

        info!(node_id = %node_id, "Node hard-deleted permanently");
        Ok(())
    }

    pub async fn find_node(&self, tenant_id: Uuid, node_id: Uuid) -> ContentResult<node::Model> {
        node::Entity::find_by_id(node_id)
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::DeletedAt.is_null())
            .one(&self.db)
            .await?
            .ok_or(ContentError::NodeNotFound(node_id))
    }

    async fn find_node_with_deleted(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
    ) -> ContentResult<node::Model> {
        node::Entity::find_by_id(node_id)
            .filter(node::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ContentError::NodeNotFound(node_id))
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, node_id = %node_id))]
    pub async fn get_node(&self, tenant_id: Uuid, node_id: Uuid) -> ContentResult<NodeResponse> {
        debug!("Fetching node");
        let node_model = self.find_node(tenant_id, node_id).await?;
        let translations = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;
        let bodies = body::Entity::find()
            .filter(body::Column::NodeId.eq(node_id))
            .all(&self.db)
            .await?;

        Ok(Self::to_response(node_model, translations, bodies))
    }

    pub async fn get_by_slug(
        &self,
        tenant_id: Uuid,
        kind: &str,
        locale: &str,
        slug: &str,
    ) -> ContentResult<Option<NodeResponse>> {
        let result = node::Entity::find()
            .inner_join(node_translation::Entity)
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::Kind.eq(kind))
            .filter(node::Column::DeletedAt.is_null())
            .filter(node_translation::Column::Locale.eq(locale))
            .filter(node_translation::Column::Slug.eq(slug))
            .one(&self.db)
            .await?;

        match result {
            Some(node_model) => Ok(Some(self.get_node(tenant_id, node_model.id).await?)),
            None => Ok(None),
        }
    }

    #[instrument(skip(self, security, filter), fields(tenant_id = %tenant_id, user_id = ?security.user_id, kind = ?filter.kind))]
    pub async fn list_nodes(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListNodesFilter,
    ) -> ContentResult<(Vec<NodeListItem>, u64)> {
        debug!(
            page = filter.page,
            per_page = filter.per_page,
            "Listing nodes"
        );
        // Scope Enforcement for List
        let resource = filter
            .kind
            .as_deref()
            .map(Self::kind_to_resource)
            .transpose()?
            .unwrap_or(Resource::Posts);
        let scope = security.get_scope(resource, Action::List);

        match scope {
            PermissionScope::All => {}
            PermissionScope::Own => {
                filter.author_id = security.user_id;
            }
            PermissionScope::None => {
                return Err(ContentError::Forbidden("Permission denied".into()));
            }
        }

        let locale = filter.locale.clone().unwrap_or_else(|| "en".to_string());
        let mut query = node::Entity::find().filter(node::Column::TenantId.eq(tenant_id));

        // Filter out soft-deleted nodes unless explicitly requested
        if !filter.include_deleted {
            query = query.filter(node::Column::DeletedAt.is_null());
        }

        if let Some(kind) = filter.kind {
            query = query.filter(node::Column::Kind.eq(kind));
        }
        if let Some(status) = filter.status {
            query = query.filter(node::Column::Status.eq(status));
        }
        if let Some(parent_id) = filter.parent_id {
            query = query.filter(node::Column::ParentId.eq(parent_id));
        }
        if let Some(author_id) = filter.author_id {
            query = query.filter(node::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = filter.category_id {
            query = query.filter(node::Column::CategoryId.eq(category_id));
        }

        let paginator = query.clone().paginate(&self.db, filter.per_page);
        let total = paginator.num_items().await?;
        let nodes = paginator.fetch_page(filter.page.saturating_sub(1)).await?;

        let node_ids: Vec<Uuid> = nodes.iter().map(|node| node.id).collect();
        let translations = node_translation::Entity::find()
            .filter(node_translation::Column::NodeId.is_in(node_ids))
            .filter(node_translation::Column::Locale.eq(locale))
            .all(&self.db)
            .await?;

        let mut translations_map = std::collections::HashMap::new();
        for translation in translations {
            translations_map.insert(translation.node_id, translation);
        }

        let items = nodes
            .into_iter()
            .map(|node| {
                let translation = translations_map.get(&node.id);
                NodeListItem {
                    id: node.id,
                    kind: node.kind,
                    status: node.status,
                    title: translation.and_then(|t| t.title.clone()),
                    slug: translation.and_then(|t| t.slug.clone()),
                    excerpt: translation.and_then(|t| t.excerpt.clone()),
                    author_id: node.author_id,
                    category_id: node.category_id,
                    metadata: node.metadata,
                    created_at: node.created_at.to_rfc3339(),
                    published_at: node.published_at.map(|date| date.to_rfc3339()),
                }
            })
            .collect();

        Ok((items, total))
    }
}

#[cfg(test)]
mod tests {
    use super::NodeService;
    use crate::dto::validation::validate_kind;
    use crate::ContentModule;
    use rustok_core::permissions::{Action, Resource};
    use rustok_core::{RusToKModule, SecurityContext, UserRole};

    #[test]
    fn kind_to_resource_maps_all_allowed_kinds_from_validation() {
        let cases = [
            ("post", Resource::Posts),
            ("page", Resource::Pages),
            ("article", Resource::Posts),
            ("custom", Resource::Posts),
            ("forum_category", Resource::ForumCategories),
            ("forum_topic", Resource::ForumTopics),
            ("forum_reply", Resource::ForumReplies),
            ("blog_post", Resource::BlogPosts),
            ("comment", Resource::Comments),
        ];

        for (kind, expected_resource) in cases {
            assert!(validate_kind(kind).is_ok(), "kind '{kind}' must stay valid");
            let actual = NodeService::kind_to_resource(kind)
                .unwrap_or_else(|_| panic!("kind '{kind}' must map to RBAC resource"));
            assert_eq!(
                actual, expected_resource,
                "invalid mapping for kind '{kind}'"
            );
        }
    }

    #[test]
    fn kind_to_resource_rejects_unknown_kind() {
        assert!(NodeService::kind_to_resource("unknown_kind").is_err());
    }

    #[test]
    fn kind_to_resource_mapping_is_covered_by_module_permissions_and_scope() {
        let module = ContentModule;
        let permissions = module.permissions();
        let admin = SecurityContext::new(UserRole::Admin, None);

        let cases = [
            ("post", Resource::Posts),
            ("page", Resource::Pages),
            ("article", Resource::Posts),
            ("custom", Resource::Posts),
            ("forum_category", Resource::ForumCategories),
            ("forum_topic", Resource::ForumTopics),
            ("forum_reply", Resource::ForumReplies),
            ("blog_post", Resource::BlogPosts),
            ("comment", Resource::Comments),
        ];

        for (kind, resource) in cases {
            for action in [
                Action::Create,
                Action::Read,
                Action::Update,
                Action::Delete,
                Action::List,
            ] {
                assert!(
                    permissions.iter().any(|p| p.resource == resource && p.action == action),
                    "ContentModule::permissions must include {resource:?}:{action:?} for kind '{kind}'"
                );
                assert_ne!(
                    admin.get_scope(resource, action),
                    rustok_core::PermissionScope::None,
                    "SecurityContext::get_scope returned None for admin on {resource:?}:{action:?}"
                );
            }
        }
    }
}

fn resolve_slug(
    slug: Option<String>,
    title: Option<&String>,
    kind: &str,
) -> ContentResult<Option<String>> {
    if kind == "comment" {
        return Ok(slug);
    }

    if let Some(slug) = slug {
        return Ok(Some(slug));
    }

    if let Some(title) = title {
        if title.trim().is_empty() {
            return Err(ContentError::Validation(
                "Slug or non-empty title must be provided".to_string(),
            ));
        }
        return Ok(Some(slug::slugify(title)));
    }

    Err(ContentError::Validation(
        "Slug or non-empty title must be provided".to_string(),
    ))
}

async fn upsert_body<C>(
    db: &C,
    node_id: Uuid,
    input: BodyInput,
    now: DateTimeWithTimeZone,
) -> ContentResult<body::Model>
where
    C: sea_orm::ConnectionTrait,
{
    let existing = body::Entity::find()
        .filter(body::Column::NodeId.eq(node_id))
        .filter(body::Column::Locale.eq(input.locale.clone()))
        .one(db)
        .await?;

    let format = input.format.unwrap_or_else(|| "markdown".to_string());

    let model = if let Some(existing) = existing {
        let mut active: body::ActiveModel = existing.into();
        if input.body.is_some() {
            active.body = Set(input.body);
        }
        active.format = Set(format);
        active.updated_at = Set(now);
        active.update(db).await?
    } else {
        body::ActiveModel {
            id: Set(rustok_core::generate_id()),
            node_id: Set(node_id),
            locale: Set(input.locale),
            body: Set(input.body),
            format: Set(format),
            updated_at: Set(now),
        }
        .insert(db)
        .await?
    };

    Ok(model)
}

impl NodeService {
    fn to_response(
        node: node::Model,
        translations: Vec<node_translation::Model>,
        bodies: Vec<body::Model>,
    ) -> NodeResponse {
        NodeResponse {
            id: node.id,
            tenant_id: node.tenant_id,
            kind: node.kind,
            status: node.status,
            parent_id: node.parent_id,
            author_id: node.author_id,
            category_id: node.category_id,
            position: node.position,
            depth: node.depth,
            reply_count: node.reply_count,
            metadata: node.metadata,
            created_at: node.created_at.to_rfc3339(),
            updated_at: node.updated_at.to_rfc3339(),
            published_at: node.published_at.map(|date| date.to_rfc3339()),
            deleted_at: node.deleted_at.map(|date| date.to_rfc3339()),
            version: node.version,
            translations: translations
                .into_iter()
                .map(|translation| NodeTranslationResponse {
                    locale: translation.locale,
                    title: translation.title,
                    slug: translation.slug,
                    excerpt: translation.excerpt,
                })
                .collect(),
            bodies: bodies
                .into_iter()
                .map(|body| BodyResponse {
                    locale: body.locale,
                    body: body.body,
                    format: body.format,
                    updated_at: body.updated_at.to_rfc3339(),
                })
                .collect(),
        }
    }
}
