use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::collections::HashMap;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::domain::{BlockType, CollaborativeDoc, EntityDisplayMode, EntityRef};
use crate::dto::{
    CreatePageDto, DocUpdateItemDto, DocumentSyncDto, PageSummaryDto,
    PageTreeNodeDto, UpdatePageDto,
};
use crate::entities::{entity_link, page, snapshot, update};
use crate::error::{DocsError, DocsResult};

pub const COMPACTION_THRESHOLD: usize = 50;

pub struct DocumentService;

impl DocumentService {
    /// Create a new collaborative document page with an initial snapshot.
    #[instrument(skip(db))]
    pub async fn create_page(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        dto: CreatePageDto,
    ) -> DocsResult<PageSummaryDto> {
        let txn = db.begin().await?;

        // 1. Slug resolution and uniqueness validation
        let raw_slug = dto.slug.unwrap_or_else(|| slugify(&dto.title));
        let slug = if raw_slug.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            raw_slug.trim().to_lowercase()
        };

        let existing = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Slug.eq(&slug))
            .filter(page::Column::DeletedAt.is_null())
            .one(&txn)
            .await?;

        if existing.is_some() {
            return Err(DocsError::SlugConflict { slug, tenant_id });
        }

        // 2. Validate parent if specified
        if let Some(parent_id) = dto.parent_id {
            let parent = page::Entity::find()
                .filter(page::Column::TenantId.eq(tenant_id))
                .filter(page::Column::Id.eq(parent_id))
                .filter(page::Column::DeletedAt.is_null())
                .one(&txn)
                .await?;

            if parent.is_none() {
                return Err(DocsError::InvalidParent(parent_id));
            }
        }

        // 3. Initialize empty CollaborativeDoc and initial snapshot
        let doc = CollaborativeDoc::new();
        let snapshot_data = doc.export_snapshot()?;
        let frontiers_json = serde_json::to_string(&doc.frontiers())?;

        let now = Utc::now();
        let page_id = Uuid::new_v4();

        let new_page = page::ActiveModel {
            id: Set(page_id),
            tenant_id: Set(tenant_id),
            parent_id: Set(dto.parent_id),
            slug: Set(slug.clone()),
            title: Set(dto.title.clone()),
            icon: Set(dto.icon.clone()),
            cover_image: Set(dto.cover_image.clone()),
            is_archived: Set(false),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        };

        let inserted_page = new_page.insert(&txn).await?;

        let initial_snapshot = snapshot::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            page_id: Set(page_id),
            seq_id: Set(0),
            snapshot_data: Set(snapshot_data),
            frontiers: Set(frontiers_json),
            created_at: Set(now.into()),
        };

        initial_snapshot.insert(&txn).await?;

        txn.commit().await?;

        info!(page_id = %page_id, tenant_id = %tenant_id, slug = %slug, "Created collaborative document page");

        Ok(PageSummaryDto {
            id: inserted_page.id,
            tenant_id: inserted_page.tenant_id,
            parent_id: inserted_page.parent_id,
            slug: inserted_page.slug,
            title: inserted_page.title,
            icon: inserted_page.icon,
            cover_image: inserted_page.cover_image,
            is_archived: inserted_page.is_archived,
            created_at: inserted_page.created_at.into(),
            updated_at: inserted_page.updated_at.into(),
        })
    }

    /// Retrieve page summary by UUID.
    pub async fn get_page(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> DocsResult<PageSummaryDto> {
        let page_model = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or(DocsError::PageNotFound(page_id))?;

        Ok(PageSummaryDto {
            id: page_model.id,
            tenant_id: page_model.tenant_id,
            parent_id: page_model.parent_id,
            slug: page_model.slug,
            title: page_model.title,
            icon: page_model.icon,
            cover_image: page_model.cover_image,
            is_archived: page_model.is_archived,
            created_at: page_model.created_at.into(),
            updated_at: page_model.updated_at.into(),
        })
    }

    /// Retrieve page summary by unique slug.
    pub async fn get_page_by_slug(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        slug: &str,
    ) -> DocsResult<PageSummaryDto> {
        let page_model = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Slug.eq(slug))
            .filter(page::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or_else(|| DocsError::SlugConflict {
                slug: slug.to_string(),
                tenant_id,
            })?;

        Ok(PageSummaryDto {
            id: page_model.id,
            tenant_id: page_model.tenant_id,
            parent_id: page_model.parent_id,
            slug: page_model.slug,
            title: page_model.title,
            icon: page_model.icon,
            cover_image: page_model.cover_image,
            is_archived: page_model.is_archived,
            created_at: page_model.created_at.into(),
            updated_at: page_model.updated_at.into(),
        })
    }

    /// List active pages for a tenant.
    pub async fn list_pages(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        include_archived: bool,
    ) -> DocsResult<Vec<PageSummaryDto>> {
        let mut query = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::DeletedAt.is_null());

        if !include_archived {
            query = query.filter(page::Column::IsArchived.eq(false));
        }

        let pages = query
            .order_by_asc(page::Column::Title)
            .all(db)
            .await?;

        Ok(pages
            .into_iter()
            .map(|p| PageSummaryDto {
                id: p.id,
                tenant_id: p.tenant_id,
                parent_id: p.parent_id,
                slug: p.slug,
                title: p.title,
                icon: p.icon,
                cover_image: p.cover_image,
                is_archived: p.is_archived,
                created_at: p.created_at.into(),
                updated_at: p.updated_at.into(),
            })
            .collect())
    }

    /// Retrieve the hierarchical navigation tree for all pages in a tenant.
    pub async fn get_page_tree(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> DocsResult<Vec<PageTreeNodeDto>> {
        let pages = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::DeletedAt.is_null())
            .order_by_asc(page::Column::Title)
            .all(db)
            .await?;

        let mut children_map: HashMap<Option<Uuid>, Vec<page::Model>> = HashMap::new();
        for p in pages {
            children_map.entry(p.parent_id).or_default().push(p);
        }

        fn build_nodes(
            parent_id: Option<Uuid>,
            map: &HashMap<Option<Uuid>, Vec<page::Model>>,
        ) -> Vec<PageTreeNodeDto> {
            match map.get(&parent_id) {
                Some(list) => list
                    .iter()
                    .map(|item| PageTreeNodeDto {
                        id: item.id,
                        slug: item.slug.clone(),
                        title: item.title.clone(),
                        icon: item.icon.clone(),
                        is_archived: item.is_archived,
                        children: build_nodes(Some(item.id), map),
                    })
                    .collect(),
                None => Vec::new(),
            }
        }

        Ok(build_nodes(None, &children_map))
    }

    /// Update page metadata.
    #[instrument(skip(db))]
    pub async fn update_page(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
        dto: UpdatePageDto,
    ) -> DocsResult<PageSummaryDto> {
        let existing = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or(DocsError::PageNotFound(page_id))?;

        let mut active: page::ActiveModel = existing.into();

        if let Some(title) = dto.title {
            active.title = Set(title);
        }

        if let Some(slug) = dto.slug {
            let normalized_slug = slug.trim().to_lowercase();
            // Validate slug uniqueness if changed
            let slug_conflict = page::Entity::find()
                .filter(page::Column::TenantId.eq(tenant_id))
                .filter(page::Column::Slug.eq(&normalized_slug))
                .filter(page::Column::Id.ne(page_id))
                .filter(page::Column::DeletedAt.is_null())
                .one(db)
                .await?;

            if slug_conflict.is_some() {
                return Err(DocsError::SlugConflict {
                    slug: normalized_slug,
                    tenant_id,
                });
            }
            active.slug = Set(normalized_slug);
        }

        if let Some(parent_opt) = dto.parent_id {
            if let Some(parent_id) = parent_opt {
                if parent_id == page_id {
                    return Err(DocsError::InvalidParent(parent_id));
                }
                let parent = page::Entity::find()
                    .filter(page::Column::TenantId.eq(tenant_id))
                    .filter(page::Column::Id.eq(parent_id))
                    .filter(page::Column::DeletedAt.is_null())
                    .one(db)
                    .await?;

                if parent.is_none() {
                    return Err(DocsError::InvalidParent(parent_id));
                }
            }
            active.parent_id = Set(parent_opt);
        }

        if let Some(icon_opt) = dto.icon {
            active.icon = Set(icon_opt);
        }

        if let Some(cover_opt) = dto.cover_image {
            active.cover_image = Set(cover_opt);
        }

        if let Some(archived) = dto.is_archived {
            active.is_archived = Set(archived);
        }

        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await?;

        Ok(PageSummaryDto {
            id: updated.id,
            tenant_id: updated.tenant_id,
            parent_id: updated.parent_id,
            slug: updated.slug,
            title: updated.title,
            icon: updated.icon,
            cover_image: updated.cover_image,
            is_archived: updated.is_archived,
            created_at: updated.created_at.into(),
            updated_at: updated.updated_at.into(),
        })
    }

    /// Delete a page (soft delete by setting deleted_at, or hard purge).
    pub async fn delete_page(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
        hard_delete: bool,
    ) -> DocsResult<()> {
        let existing = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or(DocsError::PageNotFound(page_id))?;

        if hard_delete {
            existing.delete(db).await?;
        } else {
            let mut active: page::ActiveModel = existing.into();
            active.deleted_at = Set(Some(Utc::now().into()));
            active.update(db).await?;
        }

        Ok(())
    }

    /// Load document state by applying the latest snapshot and replaying subsequent updates.
    #[instrument(skip(db))]
    pub async fn load_document(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> DocsResult<(CollaborativeDoc, i64)> {
        // 1. Verify page existence
        let _ = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or(DocsError::PageNotFound(page_id))?;

        // 2. Load latest snapshot
        let latest_snapshot = snapshot::Entity::find()
            .filter(snapshot::Column::TenantId.eq(tenant_id))
            .filter(snapshot::Column::PageId.eq(page_id))
            .order_by_desc(snapshot::Column::SeqId)
            .one(db)
            .await?;

        let (doc, base_seq) = match latest_snapshot {
            Some(snap) => {
                let doc = CollaborativeDoc::from_snapshot(&snap.snapshot_data)?;
                (doc, snap.seq_id)
            }
            None => (CollaborativeDoc::new(), 0),
        };

        // 3. Replay append-only delta updates since snapshot
        let updates = update::Entity::find()
            .filter(update::Column::TenantId.eq(tenant_id))
            .filter(update::Column::PageId.eq(page_id))
            .filter(update::Column::SeqId.gt(base_seq))
            .order_by_asc(update::Column::SeqId)
            .all(db)
            .await?;

        let mut latest_seq = base_seq;
        for u in updates {
            doc.apply_delta(&u.change_data)?;
            if u.seq_id > latest_seq {
                latest_seq = u.seq_id;
            }
        }

        Ok((doc, latest_seq))
    }

    /// Append a batch of binary CRDT updates monotonically, compacting when threshold is reached.
    #[instrument(skip(db, deltas))]
    pub async fn append_updates(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
        deltas: Vec<Vec<u8>>,
    ) -> DocsResult<i64> {
        if deltas.is_empty() {
            let (_, latest_seq) = Self::load_document(db, tenant_id, page_id).await?;
            return Ok(latest_seq);
        }

        let txn = db.begin().await?;

        // 1. Ensure page exists
        let page_model = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant_id))
            .filter(page::Column::Id.eq(page_id))
            .filter(page::Column::DeletedAt.is_null())
            .one(&txn)
            .await?
            .ok_or(DocsError::PageNotFound(page_id))?;

        // 2. Determine current latest seq_id
        let latest_update = update::Entity::find()
            .filter(update::Column::TenantId.eq(tenant_id))
            .filter(update::Column::PageId.eq(page_id))
            .order_by_desc(update::Column::SeqId)
            .one(&txn)
            .await?;

        let mut current_seq = match latest_update {
            Some(u) => u.seq_id,
            None => {
                let snap = snapshot::Entity::find()
                    .filter(snapshot::Column::TenantId.eq(tenant_id))
                    .filter(snapshot::Column::PageId.eq(page_id))
                    .order_by_desc(snapshot::Column::SeqId)
                    .one(&txn)
                    .await?;
                snap.map(|s| s.seq_id).unwrap_or(0)
            }
        };

        // 3. Insert each delta with monotonic seq_id
        let now = Utc::now();
        for delta in deltas {
            current_seq += 1;
            let record = update::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                page_id: Set(page_id),
                seq_id: Set(current_seq),
                change_data: Set(delta),
                created_at: Set(now.into()),
            };
            record.insert(&txn).await?;
        }

        // 4. Update page timestamp
        let mut page_am: page::ActiveModel = page_model.into();
        page_am.updated_at = Set(now.into());
        page_am.update(&txn).await?;

        txn.commit().await?;

        // 5. Check if compaction is needed
        let uncompacted_count = update::Entity::find()
            .filter(update::Column::TenantId.eq(tenant_id))
            .filter(update::Column::PageId.eq(page_id))
            .count(db)
            .await?;

        if uncompacted_count >= COMPACTION_THRESHOLD as u64 {
            info!(
                page_id = %page_id,
                uncompacted = uncompacted_count,
                "Triggering snapshot compaction for collaborative document"
            );
            let _ = Self::compact_snapshot(db, tenant_id, page_id).await;
        }

        // 6. Refresh entity backlinks ledger
        let (doc, _) = Self::load_document(db, tenant_id, page_id).await?;
        Self::sync_entity_links(db, tenant_id, page_id, &doc).await?;

        Ok(current_seq)
    }

    /// Compact all deltas into a fresh binary snapshot and prune old update records.
    #[instrument(skip(db))]
    pub async fn compact_snapshot(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> DocsResult<()> {
        let (doc, latest_seq) = Self::load_document(db, tenant_id, page_id).await?;

        let snapshot_data = doc.export_snapshot()?;
        let frontiers_json = serde_json::to_string(&doc.frontiers())?;

        let txn = db.begin().await?;

        let now = Utc::now();
        let new_snapshot = snapshot::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            page_id: Set(page_id),
            seq_id: Set(latest_seq),
            snapshot_data: Set(snapshot_data),
            frontiers: Set(frontiers_json),
            created_at: Set(now.into()),
        };

        new_snapshot.insert(&txn).await?;

        // Prune updates with seq_id <= latest_seq
        update::Entity::delete_many()
            .filter(update::Column::TenantId.eq(tenant_id))
            .filter(update::Column::PageId.eq(page_id))
            .filter(update::Column::SeqId.lte(latest_seq))
            .exec(&txn)
            .await?;

        txn.commit().await?;

        info!(page_id = %page_id, seq_id = latest_seq, "Document snapshot compacted successfully");

        Ok(())
    }

    /// Retrieve sync payload for a peer or client, supporting differential updates from a client seq_id.
    pub async fn get_sync_payload(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
        client_seq_id: Option<i64>,
    ) -> DocsResult<DocumentSyncDto> {
        let latest_snapshot = snapshot::Entity::find()
            .filter(snapshot::Column::TenantId.eq(tenant_id))
            .filter(snapshot::Column::PageId.eq(page_id))
            .order_by_desc(snapshot::Column::SeqId)
            .one(db)
            .await?;

        let (snapshot_bytes, snapshot_seq) = match latest_snapshot {
            Some(snap) => (Some(snap.snapshot_data), snap.seq_id),
            None => (None, 0),
        };

        let start_seq = match client_seq_id {
            Some(seq) if seq >= snapshot_seq => seq,
            _ => snapshot_seq,
        };

        let updates = update::Entity::find()
            .filter(update::Column::TenantId.eq(tenant_id))
            .filter(update::Column::PageId.eq(page_id))
            .filter(update::Column::SeqId.gt(start_seq))
            .order_by_asc(update::Column::SeqId)
            .all(db)
            .await?;

        let latest_seq = updates
            .last()
            .map(|u| u.seq_id)
            .unwrap_or(start_seq);

        let update_items = updates
            .into_iter()
            .map(|u| DocUpdateItemDto {
                seq_id: u.seq_id,
                data: u.change_data,
            })
            .collect();

        let need_snapshot = client_seq_id.map(|s| s < snapshot_seq).unwrap_or(true);

        Ok(DocumentSyncDto {
            page_id,
            latest_seq_id: latest_seq,
            snapshot: if need_snapshot { snapshot_bytes } else { None },
            updates: update_items,
        })
    }

    /// Synchronize reverse backlinks into docs_entity_links for all embedded UER entities.
    #[instrument(skip(db, doc))]
    pub async fn sync_entity_links(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
        doc: &CollaborativeDoc,
    ) -> DocsResult<()> {
        let blocks = doc.get_blocks()?;
        let mut links: Vec<(String, String, String, String)> = Vec::new();

        for block in blocks {
            if let BlockType::EntityEmbed(eref) = block.block_type {
                let mode = match eref.display_mode {
                    EntityDisplayMode::Inline => "inline",
                    EntityDisplayMode::Card => "card",
                    EntityDisplayMode::Embed => "embed",
                    EntityDisplayMode::DatabaseRow => "database_row",
                };
                links.push((
                    eref.target_module,
                    eref.target_entity_type,
                    eref.target_entity_id,
                    mode.to_string(),
                ));
            }
        }

        let txn = db.begin().await?;

        // 1. Delete previous backlinks for this source page
        entity_link::Entity::delete_many()
            .filter(entity_link::Column::TenantId.eq(tenant_id))
            .filter(entity_link::Column::SourcePageId.eq(page_id))
            .exec(&txn)
            .await?;

        // 2. Insert extracted backlinks
        let now = Utc::now();
        for (module, entity_type, entity_id, mode) in links {
            let item = entity_link::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                source_page_id: Set(page_id),
                target_module: Set(module),
                target_entity_type: Set(entity_type),
                target_entity_id: Set(entity_id),
                display_mode: Set(mode),
                created_at: Set(now.into()),
            };
            item.insert(&txn).await?;
        }

        txn.commit().await?;

        Ok(())
    }

    /// Render page to Markdown projection.
    pub async fn render_markdown(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        page_id: Uuid,
    ) -> DocsResult<String> {
        let (doc, _) = Self::load_document(db, tenant_id, page_id).await?;
        doc.render_markdown()
    }
}

fn slugify(text: &str) -> String {
    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "page".to_string()
    } else {
        trimmed.to_string()
    }
}
