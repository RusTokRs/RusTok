use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
    PLATFORM_FALLBACK_LOCALE,
};
use rustok_core::{prepare_content_payload, Action, Resource, SecurityContext};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;

use crate::dto::{
    CreatePostInput, PostListQuery, PostListResponse, PostResponse, PostSummary, UpdatePostInput,
};
use crate::entities::{blog_post, blog_post_translation};
use crate::error::{BlogError, BlogResult};
use crate::services::category::CategoryService;
use crate::services::rbac::{
    can_read_non_public_posts, enforce_create_author, enforce_owned_scope, enforce_scope,
};
use crate::services::tag::{find_post_ids_by_tag, load_post_tags_map, sync_post_tags_in_tx};
use crate::state_machine::BlogPostStatus;

const CHANNEL_VISIBILITY_KEY: &str = "channel_visibility";
const ALLOWED_CHANNEL_SLUGS_KEY: &str = "allowed_channel_slugs";

pub struct PostService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

struct ResolvedTranslationRecord<'a> {
    translation: Option<&'a blog_post_translation::Model>,
    effective_locale: String,
}

impl PostService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self, security, input))]
    pub async fn create_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreatePostInput,
    ) -> BlogResult<Uuid> {
        let CreatePostInput {
            locale,
            title,
            body,
            body_format,
            content_json,
            excerpt,
            slug,
            publish,
            tags,
            category_id,
            featured_image_url,
            seo_title,
            seo_description,
            channel_slugs,
            metadata,
        } = input;

        validate_title(&title)?;
        validate_locale(&locale)?;
        validate_tags(&tags)?;

        let author_id = enforce_create_author(&security, Resource::BlogPosts, Action::Create)?;
        let prepared_body = prepare_content_payload(
            Some(&body_format),
            Some(&body),
            content_json.as_ref(),
            &locale,
            "Body",
        )
        .map_err(BlogError::validation)?;

        let slug = normalize_slug(slug.as_deref().unwrap_or(&title));
        if slug.is_empty() {
            return Err(BlogError::validation("Slug cannot be empty"));
        }

        let now = chrono::Utc::now();
        let metadata = build_post_metadata(
            metadata,
            Some(tags.clone()),
            category_id,
            featured_image_url.clone(),
            seo_title.clone(),
            seo_description.clone(),
            channel_slugs.as_deref(),
        );

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        self.ensure_slug_unique_in_tx(&txn, tenant_id, &slug, None)
            .await?;
        if let Some(category_id) = category_id {
            CategoryService::ensure_exists_in_tx(&txn, tenant_id, category_id).await?;
        }

        let post_id = Uuid::new_v4();
        let status = if publish {
            BlogPostStatus::Published
        } else {
            BlogPostStatus::Draft
        };

        blog_post::ActiveModel {
            id: Set(post_id),
            tenant_id: Set(tenant_id),
            author_id: Set(author_id),
            category_id: Set(category_id),
            status: Set(status_to_storage(status).to_string()),
            slug: Set(slug),
            metadata: Set(metadata),
            featured_image_url: Set(featured_image_url),
            published_at: Set(if publish { Some(now.into()) } else { None }),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            archived_at: Set(None),
            comment_count: Set(0),
            view_count: Set(0),
            version: Set(1),
        }
        .insert(&txn)
        .await
        .map_err(BlogError::from)?;

        blog_post_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            post_id: Set(post_id),
            locale: Set(normalize_locale(&locale)?),
            title: Set(title),
            excerpt: Set(excerpt),
            seo_title: Set(seo_title),
            seo_description: Set(seo_description),
            body: Set(prepared_body.body),
            body_format: Set(prepared_body.format),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await
        .map_err(BlogError::from)?;

        sync_post_tags_in_tx(&txn, tenant_id, post_id, &tags, &locale).await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostCreated {
                    post_id,
                    author_id: Some(author_id),
                    locale,
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(post_id)
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        input: UpdatePostInput,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;
        if let Some(expected_version) = input.version {
            if post.version != expected_version {
                return Err(BlogError::validation("Version mismatch"));
            }
        }

        let locale = input
            .locale
            .clone()
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        validate_optional_title(input.title.as_deref())?;
        if let Some(ref tags) = input.tags {
            validate_tags(tags)?;
        }

        let mut metadata = post.metadata.clone();
        if let Some(override_metadata) = input.metadata.clone() {
            merge_metadata(&mut metadata, override_metadata);
        }
        if let Some(ref tags) = input.tags {
            set_metadata_array(&mut metadata, "tags", tags.clone());
        }
        if let Some(category_id) = input.category_id {
            set_metadata_uuid(&mut metadata, "category_id", category_id);
        }
        if let Some(ref url) = input.featured_image_url {
            set_metadata_string(&mut metadata, "featured_image_url", url);
        }
        if let Some(ref seo_title) = input.seo_title {
            set_metadata_string(&mut metadata, "seo_title", seo_title);
        }
        if let Some(ref seo_description) = input.seo_description {
            set_metadata_string(&mut metadata, "seo_description", seo_description);
        }
        apply_channel_visibility_metadata(&mut metadata, input.channel_slugs.as_deref());

        let mut prepared_body = None;
        if input.body.is_some() || input.content_json.is_some() || input.body_format.is_some() {
            prepared_body = Some(
                prepare_content_payload(
                    input.body_format.as_deref(),
                    input.body.as_deref(),
                    input.content_json.as_ref(),
                    &locale,
                    "Body",
                )
                .map_err(BlogError::validation)?,
            );
        }

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let now = chrono::Utc::now();

        let mut post_active: blog_post::ActiveModel = post.clone().into();
        if let Some(ref slug) = input.slug {
            let normalized = normalize_slug(slug);
            if normalized.is_empty() {
                return Err(BlogError::validation("Slug cannot be empty"));
            }
            self.ensure_slug_unique_in_tx(&txn, tenant_id, &normalized, Some(post_id))
                .await?;
            post_active.slug = Set(normalized);
        }
        if input.category_id.is_some() {
            if let Some(category_id) = input.category_id {
                CategoryService::ensure_exists_in_tx(&txn, tenant_id, category_id).await?;
            }
            post_active.category_id = Set(input.category_id);
        }
        if input.featured_image_url.is_some() {
            post_active.featured_image_url = Set(input.featured_image_url.clone());
        }
        post_active.metadata = Set(metadata);
        post_active.updated_at = Set(now.into());
        post_active.version = Set(post.version + 1);
        post_active.update(&txn).await.map_err(BlogError::from)?;

        self.upsert_translation_in_tx(
            &txn,
            post_id,
            &locale,
            input.title,
            input.excerpt,
            input.seo_title,
            input.seo_description,
            prepared_body,
            now,
        )
        .await?;

        if let Some(ref tags) = input.tags {
            sync_post_tags_in_tx(&txn, tenant_id, post_id, tags, &locale).await?;
        }

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostUpdated { post_id, locale },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn publish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.status = Set(status_to_storage(BlogPostStatus::Published).to_string());
        active.published_at = Set(Some(now.into()));
        active.archived_at = Set(None);
        active.updated_at = Set(now.into());
        active.version = Set(post.version + 1);
        active.update(&txn).await.map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostPublished {
                    post_id,
                    author_id: Some(post.author_id),
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn unpublish_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.status = Set(status_to_storage(BlogPostStatus::Draft).to_string());
        active.published_at = Set(None);
        active.updated_at = Set(now.into());
        active.version = Set(post.version + 1);
        active.update(&txn).await.map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostUnpublished { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn archive_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        reason: Option<String>,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Publish,
            post.author_id,
        )?;
        let now = chrono::Utc::now();
        let txn = self.db.begin().await.map_err(BlogError::from)?;

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.status = Set(status_to_storage(BlogPostStatus::Archived).to_string());
        active.archived_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.version = Set(post.version + 1);
        active.update(&txn).await.map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostArchived {
                    post_id,
                    reason: reason.clone(),
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self, security))]
    pub async fn delete_post(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        let post = self.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Delete,
            post.author_id,
        )?;
        if storage_to_status(&post.status)? == BlogPostStatus::Published {
            return Err(BlogError::CannotDeletePublished);
        }

        let txn = self.db.begin().await.map_err(BlogError::from)?;
        blog_post::Entity::delete_by_id(post_id)
            .exec(&txn)
            .await
            .map_err(BlogError::from)?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::BlogPostDeleted { post_id },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await.map_err(BlogError::from)?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_post(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        locale: &str,
    ) -> BlogResult<PostResponse> {
        self.get_post_with_locale_fallback(tenant_id, security, post_id, locale, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_post_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        post_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostResponse> {
        enforce_scope(&security, Resource::BlogPosts, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let post = self.find_post(tenant_id, post_id).await?;
        if !can_read_non_public_posts(&security)
            && storage_to_status(&post.status)? != BlogPostStatus::Published
        {
            return Err(BlogError::forbidden("Permission denied"));
        }
        let translations = self.load_translations(post_id).await?;
        self.build_post_response(post, translations, &locale, fallback_locale.as_deref())
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_post_by_slug(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
    ) -> BlogResult<Option<PostResponse>> {
        self.get_post_by_slug_with_locale_fallback(tenant_id, security, locale, slug, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn get_post_by_slug_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        slug: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<PostResponse>> {
        enforce_scope(&security, Resource::BlogPosts, Action::Read)?;
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let Some(post) = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Slug.eq(normalize_slug(slug)))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
        else {
            return Ok(None);
        };

        if storage_to_status(&post.status)? != BlogPostStatus::Published
            && !can_read_non_public_posts(&security)
        {
            return Ok(None);
        }

        let translations = self.load_translations(post.id).await?;
        self.build_post_response(post, translations, &locale, fallback_locale.as_deref())
            .await
            .map(Some)
    }

    #[instrument(skip(self, security))]
    pub async fn list_posts(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
    ) -> BlogResult<PostListResponse> {
        self.list_posts_with_locale_fallback(tenant_id, security, query, None)
            .await
    }

    #[instrument(skip(self))]
    pub async fn list_posts_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostListResponse> {
        enforce_scope(&security, Resource::BlogPosts, Action::List)?;
        let locale = query
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let tag_filter = query.tag.clone();
        let mut select =
            blog_post::Entity::find().filter(blog_post::Column::TenantId.eq(tenant_id));

        if let Some(ref tag) = tag_filter {
            let tagged_post_ids = find_post_ids_by_tag(&self.db, tenant_id, tag).await?;
            if tagged_post_ids.is_empty() {
                return Ok(PostListResponse::new(Vec::new(), 0, &query));
            }
            select = select.filter(blog_post::Column::Id.is_in(tagged_post_ids));
        }

        if let Some(status) = query.status {
            select = select.filter(blog_post::Column::Status.eq(status_to_storage(status)));
        } else if !can_read_non_public_posts(&security) {
            select = select
                .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)));
        }
        if !can_read_non_public_posts(&security)
            && matches!(query.status, Some(status) if status != BlogPostStatus::Published)
        {
            return Ok(PostListResponse::new(Vec::new(), 0, &query));
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(blog_post::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(blog_post::Column::CategoryId.eq(category_id));
        }

        select = apply_post_sort(select, &query);

        let paginator = select.paginate(&self.db, query.per_page() as u64);
        let total = paginator.num_items().await.map_err(BlogError::from)?;
        let posts = paginator
            .fetch_page((query.page().saturating_sub(1)) as u64)
            .await
            .map_err(BlogError::from)?;
        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();

        let translations_map = self.load_translations_map(&post_ids).await?;
        let tags_map =
            load_post_tags_map(&self.db, &post_ids, &locale, fallback_locale.as_deref()).await?;

        let mut items = Vec::with_capacity(posts.len());
        for post in posts {
            let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
            let resolved =
                resolve_translation_record(&translations, &locale, fallback_locale.as_deref());
            let translation = resolved.translation;
            let tags = tags_map
                .get(&post.id)
                .cloned()
                .unwrap_or_else(|| extract_tags(&post.metadata));

            items.push(PostSummary {
                id: post.id,
                title: translation
                    .map(|item| item.title.clone())
                    .unwrap_or_default(),
                slug: post.slug.clone(),
                locale: locale.clone(),
                effective_locale: resolved.effective_locale,
                excerpt: translation.and_then(|item| item.excerpt.clone()),
                status: storage_to_status(&post.status)?,
                author_id: post.author_id,
                author_name: None,
                category_id: post.category_id,
                category_name: None,
                tags,
                featured_image_url: post.featured_image_url.clone(),
                channel_slugs: extract_channel_slugs(&post.metadata),
                comment_count: post.comment_count as i64,
                published_at: post.published_at.map(Into::into),
                created_at: post.created_at.into(),
            });
        }

        Ok(PostListResponse::new(items, total, &query))
    }

    pub async fn get_posts_by_tag(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        tag: String,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            tag: Some(tag),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub async fn get_posts_by_category(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        category_id: Uuid,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            category_id: Some(category_id),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    pub async fn get_posts_by_author(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        author_id: Uuid,
        page: u32,
        per_page: u32,
    ) -> BlogResult<PostListResponse> {
        let query = PostListQuery {
            author_id: Some(author_id),
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        self.list_posts(tenant_id, security, query).await
    }

    async fn find_post(&self, tenant_id: Uuid, post_id: Uuid) -> BlogResult<blog_post::Model> {
        blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?
            .ok_or(BlogError::PostNotFound(post_id))
    }

    async fn load_translations(
        &self,
        post_id: Uuid,
    ) -> BlogResult<Vec<blog_post_translation::Model>> {
        blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .all(&self.db)
            .await
            .map_err(BlogError::from)
    }

    async fn load_translations_map(
        &self,
        post_ids: &[Uuid],
    ) -> BlogResult<HashMap<Uuid, Vec<blog_post_translation::Model>>> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let translations = blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.is_in(post_ids.to_vec()))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let mut map: HashMap<Uuid, Vec<blog_post_translation::Model>> = HashMap::new();
        for translation in translations {
            map.entry(translation.post_id)
                .or_default()
                .push(translation);
        }
        Ok(map)
    }

    async fn ensure_slug_unique_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        slug: &str,
        exclude_post_id: Option<Uuid>,
    ) -> BlogResult<()> {
        let mut query = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Slug.eq(slug));
        if let Some(exclude_post_id) = exclude_post_id {
            query = query.filter(blog_post::Column::Id.ne(exclude_post_id));
        }

        if query.one(txn).await.map_err(BlogError::from)?.is_some() {
            return Err(BlogError::duplicate_slug(
                slug.to_string(),
                PLATFORM_FALLBACK_LOCALE.to_string(),
            ));
        }

        Ok(())
    }

    async fn upsert_translation_in_tx(
        &self,
        txn: &DatabaseTransaction,
        post_id: Uuid,
        locale: &str,
        title: Option<String>,
        excerpt: Option<String>,
        seo_title: Option<String>,
        seo_description: Option<String>,
        prepared_body: Option<rustok_core::PreparedContent>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> BlogResult<()> {
        let locale = normalize_locale(locale)?;
        let existing = blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .filter(blog_post_translation::Column::Locale.eq(locale.as_str()))
            .one(txn)
            .await
            .map_err(BlogError::from)?;

        match existing {
            Some(existing) => {
                let mut active: blog_post_translation::ActiveModel = existing.clone().into();
                if let Some(title) = title {
                    validate_title(&title)?;
                    active.title = Set(title);
                }
                if excerpt.is_some() {
                    active.excerpt = Set(excerpt);
                }
                if seo_title.is_some() {
                    active.seo_title = Set(seo_title);
                }
                if seo_description.is_some() {
                    active.seo_description = Set(seo_description);
                }
                if let Some(prepared_body) = prepared_body {
                    active.body = Set(prepared_body.body);
                    active.body_format = Set(prepared_body.format);
                }
                active.updated_at = Set(now.into());
                active.update(txn).await.map_err(BlogError::from)?;
            }
            None => {
                let baseline = self
                    .translation_seed_in_tx(txn, post_id)
                    .await
                    .map_err(BlogError::from)?;
                let title = title
                    .or_else(|| baseline.as_ref().map(|item| item.title.clone()))
                    .ok_or_else(|| BlogError::validation("Title is required for a new locale"))?;
                validate_title(&title)?;
                let excerpt =
                    excerpt.or_else(|| baseline.as_ref().and_then(|item| item.excerpt.clone()));
                let seo_title =
                    seo_title.or_else(|| baseline.as_ref().and_then(|item| item.seo_title.clone()));
                let seo_description = seo_description.or_else(|| {
                    baseline
                        .as_ref()
                        .and_then(|item| item.seo_description.clone())
                });
                let prepared_body = prepared_body
                    .or_else(|| {
                        baseline.as_ref().map(|item| rustok_core::PreparedContent {
                            body: item.body.clone(),
                            format: item.body_format.clone(),
                        })
                    })
                    .ok_or_else(|| BlogError::validation("Body is required for a new locale"))?;

                blog_post_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    post_id: Set(post_id),
                    locale: Set(locale),
                    title: Set(title),
                    excerpt: Set(excerpt),
                    seo_title: Set(seo_title),
                    seo_description: Set(seo_description),
                    body: Set(prepared_body.body),
                    body_format: Set(prepared_body.format),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await
                .map_err(BlogError::from)?;
            }
        }

        Ok(())
    }

    async fn translation_seed_in_tx(
        &self,
        txn: &DatabaseTransaction,
        post_id: Uuid,
    ) -> Result<Option<blog_post_translation::Model>, sea_orm::DbErr> {
        blog_post_translation::Entity::find()
            .filter(blog_post_translation::Column::PostId.eq(post_id))
            .order_by_asc(blog_post_translation::Column::CreatedAt)
            .one(txn)
            .await
    }

    async fn build_post_response(
        &self,
        post: blog_post::Model,
        translations: Vec<blog_post_translation::Model>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostResponse> {
        let tags_map = load_post_tags_map(&self.db, &[post.id], locale, fallback_locale).await?;
        let resolved = resolve_translation_record(&translations, locale, fallback_locale);
        let translation = resolved.translation;
        let body = translation
            .map(|item| item.body.clone())
            .unwrap_or_default();
        let body_format = translation
            .map(|item| item.body_format.clone())
            .unwrap_or_else(|| "markdown".to_string());
        let content_json = if body_format == "rt_json_v1" {
            serde_json::from_str(&body).ok()
        } else {
            None
        };

        Ok(PostResponse {
            id: post.id,
            tenant_id: post.tenant_id,
            author_id: post.author_id,
            title: translation
                .map(|item| item.title.clone())
                .unwrap_or_default(),
            slug: post.slug,
            requested_locale: locale.to_string(),
            locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            available_locales: available_locales_from(&translations, |item| item.locale.as_str()),
            body,
            body_format,
            content_json,
            excerpt: translation.and_then(|item| item.excerpt.clone()),
            status: storage_to_status(&post.status)?,
            category_id: post.category_id,
            category_name: None,
            tags: tags_map
                .get(&post.id)
                .cloned()
                .unwrap_or_else(|| extract_tags(&post.metadata)),
            featured_image_url: post.featured_image_url,
            seo_title: translation.and_then(|item| item.seo_title.clone()),
            seo_description: translation.and_then(|item| item.seo_description.clone()),
            channel_slugs: extract_channel_slugs(&post.metadata),
            metadata: post.metadata,
            comment_count: post.comment_count as i64,
            view_count: post.view_count as i64,
            created_at: post.created_at.into(),
            updated_at: post.updated_at.into(),
            published_at: post.published_at.map(Into::into),
            version: post.version,
        })
    }
}

fn resolve_translation_record<'a>(
    translations: &'a [blog_post_translation::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedTranslationRecord<'a> {
    let resolved =
        resolve_by_locale_with_fallback(translations, requested, fallback_locale, |item| {
            item.locale.as_str()
        });
    ResolvedTranslationRecord {
        translation: resolved.item,
        effective_locale: resolved.effective_locale,
    }
}

fn apply_post_sort(
    mut select: sea_orm::Select<blog_post::Entity>,
    query: &PostListQuery,
) -> sea_orm::Select<blog_post::Entity> {
    let ascending = matches!(query.sort_order.as_deref(), Some("asc" | "ASC"));
    match query.sort_by.as_deref() {
        Some("published_at") => {
            if ascending {
                select = select.order_by_asc(blog_post::Column::PublishedAt);
            } else {
                select = select.order_by_desc(blog_post::Column::PublishedAt);
            }
        }
        Some("updated_at") => {
            if ascending {
                select = select.order_by_asc(blog_post::Column::UpdatedAt);
            } else {
                select = select.order_by_desc(blog_post::Column::UpdatedAt);
            }
        }
        _ => {
            if ascending {
                select = select.order_by_asc(blog_post::Column::CreatedAt);
            } else {
                select = select.order_by_desc(blog_post::Column::CreatedAt);
            }
        }
    }
    select
}

fn validate_title(title: &str) -> BlogResult<()> {
    if title.trim().is_empty() {
        return Err(BlogError::validation("Title cannot be empty"));
    }
    if title.len() > 512 {
        return Err(BlogError::validation("Title cannot exceed 512 characters"));
    }
    Ok(())
}

fn validate_optional_title(title: Option<&str>) -> BlogResult<()> {
    if let Some(title) = title {
        validate_title(title)?;
    }
    Ok(())
}

fn validate_locale(locale: &str) -> BlogResult<()> {
    if locale.trim().is_empty() {
        return Err(BlogError::validation("Locale cannot be empty"));
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> BlogResult<()> {
    if tags.len() > 20 {
        return Err(BlogError::validation("Cannot have more than 20 tags"));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    normalize_locale_code(locale).ok_or_else(|| BlogError::validation("Invalid locale"))
}

fn normalize_slug(slug: &str) -> String {
    let mut normalized = String::with_capacity(slug.len());
    let mut previous_dash = false;
    for ch in slug.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn build_post_metadata(
    metadata: Option<Value>,
    tags: Option<Vec<String>>,
    category_id: Option<Uuid>,
    featured_image_url: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
    channel_slugs: Option<&[String]>,
) -> Value {
    let mut metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    if let Some(tags) = tags {
        set_metadata_array(&mut metadata, "tags", tags);
    }
    if let Some(category_id) = category_id {
        set_metadata_uuid(&mut metadata, "category_id", category_id);
    }
    if let Some(featured_image_url) = featured_image_url {
        set_metadata_string(&mut metadata, "featured_image_url", &featured_image_url);
    }
    if let Some(seo_title) = seo_title {
        set_metadata_string(&mut metadata, "seo_title", &seo_title);
    }
    if let Some(seo_description) = seo_description {
        set_metadata_string(&mut metadata, "seo_description", &seo_description);
    }
    apply_channel_visibility_metadata(&mut metadata, channel_slugs);
    metadata
}

fn set_metadata_array(metadata: &mut Value, key: &str, values: Vec<String>) {
    ensure_metadata_object(metadata).insert(key.to_string(), serde_json::json!(values));
}

fn set_metadata_uuid(metadata: &mut Value, key: &str, value: Uuid) {
    ensure_metadata_object(metadata).insert(key.to_string(), serde_json::json!(value));
}

fn set_metadata_string(metadata: &mut Value, key: &str, value: &str) {
    ensure_metadata_object(metadata).insert(key.to_string(), serde_json::json!(value));
}

fn ensure_metadata_object(metadata: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !metadata.is_object() {
        *metadata = serde_json::json!({});
    }
    metadata
        .as_object_mut()
        .expect("metadata must be an object after normalization")
}

fn merge_metadata(base: &mut Value, patch: Value) {
    match patch {
        Value::Object(patch_map) => {
            let base_map = ensure_metadata_object(base);
            for (key, value) in patch_map {
                base_map.insert(key, value);
            }
        }
        other => *base = other,
    }
}

fn apply_channel_visibility_metadata(metadata: &mut Value, channel_slugs: Option<&[String]>) {
    let Some(channel_slugs) = channel_slugs else {
        return;
    };

    let normalized = normalize_channel_slugs(channel_slugs);
    let object = ensure_metadata_object(metadata);
    if normalized.is_empty() {
        object.remove(CHANNEL_VISIBILITY_KEY);
        return;
    }

    object.insert(
        CHANNEL_VISIBILITY_KEY.to_string(),
        serde_json::json!({
            ALLOWED_CHANNEL_SLUGS_KEY: normalized,
        }),
    );
}

pub(crate) fn extract_channel_slugs(metadata: &Value) -> Vec<String> {
    metadata
        .get(CHANNEL_VISIBILITY_KEY)
        .and_then(|value| value.get(ALLOWED_CHANNEL_SLUGS_KEY))
        .and_then(|value| value.as_array())
        .map(|items| {
            normalize_channel_slugs(
                &items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default()
}

pub(crate) fn is_post_visible_for_channel(metadata: &Value, channel_slug: Option<&str>) -> bool {
    let allowed_channel_slugs = extract_channel_slugs(metadata);
    if allowed_channel_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = channel_slug else {
        return false;
    };

    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && allowed_channel_slugs.iter().any(|item| item == &normalized)
}

fn normalize_channel_slugs(channel_slugs: &[String]) -> Vec<String> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn extract_tags(metadata: &Value) -> Vec<String> {
    metadata
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn storage_to_status(status: &str) -> BlogResult<BlogPostStatus> {
    match status {
        "draft" => Ok(BlogPostStatus::Draft),
        "published" => Ok(BlogPostStatus::Published),
        "archived" => Ok(BlogPostStatus::Archived),
        other => Err(BlogError::validation(format!(
            "Unknown blog post status: {other}"
        ))),
    }
}

fn status_to_storage(status: BlogPostStatus) -> &'static str {
    match status {
        BlogPostStatus::Draft => "draft",
        BlogPostStatus::Published => "published",
        BlogPostStatus::Archived => "archived",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use rustok_core::{MemoryTransport, SecurityContext, UserRole};
    use sea_orm::{ConnectOptions, Database, DatabaseConnection};
    use sea_orm_migration::SchemaManager;

    async fn setup_test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:blog_service_post_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);

        Database::connect(opts)
            .await
            .expect("failed to connect blog test sqlite database")
    }

    async fn ensure_blog_schema(db: &DatabaseConnection) {
        let manager = SchemaManager::new(db);
        for migration in crate::migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("blog migration should apply");
        }
    }

    #[test]
    fn post_list_query_defaults() {
        let query = PostListQuery::default();
        assert_eq!(query.page(), 1);
        assert_eq!(query.per_page(), 20);
        assert_eq!(query.offset(), 0);
    }

    #[test]
    fn post_list_query_clamps_bounds() {
        let query = PostListQuery {
            page: Some(0),
            per_page: Some(200),
            ..Default::default()
        };
        assert_eq!(query.page(), 1);
        assert_eq!(query.per_page(), 100);
    }

    #[test]
    fn channel_visibility_normalizes_and_filters_blog_metadata() {
        let mut metadata = serde_json::json!({});
        apply_channel_visibility_metadata(
            &mut metadata,
            Some(&[" Web ".to_string(), "mobile".to_string(), "web".to_string()]),
        );

        assert_eq!(
            extract_channel_slugs(&metadata),
            vec!["mobile".to_string(), "web".to_string()]
        );
        assert!(is_post_visible_for_channel(&metadata, Some("web")));
        assert!(!is_post_visible_for_channel(&metadata, Some("storefront")));
        assert!(!is_post_visible_for_channel(&metadata, None));
    }

    #[test]
    fn slug_normalization_is_stable() {
        assert_eq!(normalize_slug("Hello, World!"), "hello-world");
        assert_eq!(normalize_slug("  many   spaces  "), "many-spaces");
    }

    #[tokio::test]
    async fn post_lifecycle_uses_blog_owned_tables() {
        let db = setup_test_db().await;
        ensure_blog_schema(&db).await;

        let transport = MemoryTransport::new();
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(transport));
        let post_service = PostService::new(db.clone(), event_bus);

        let tenant_id = Uuid::new_v4();
        let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

        let post_id = post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: "Draft Post".to_string(),
                    body: "Content".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    excerpt: None,
                    slug: Some("draft-post".to_string()),
                    publish: false,
                    tags: vec!["rust".to_string()],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await
            .expect("post should be created");

        let draft = post_service
            .get_post(tenant_id, admin.clone(), post_id, "en")
            .await
            .expect("draft should be readable");
        assert_eq!(draft.status, BlogPostStatus::Draft);
        assert_eq!(draft.tags, vec!["rust"]);

        post_service
            .publish_post(tenant_id, post_id, admin.clone())
            .await
            .expect("post should publish");

        let published = post_service
            .get_post(tenant_id, admin.clone(), post_id, "en")
            .await
            .expect("published should be readable");
        assert_eq!(published.status, BlogPostStatus::Published);
        assert_eq!(published.slug, "draft-post");
        assert!(published.published_at.is_some());
    }

    #[tokio::test]
    async fn customer_cannot_create_or_read_draft_posts() {
        let db = setup_test_db().await;
        ensure_blog_schema(&db).await;

        let transport = MemoryTransport::new();
        let _receiver = transport.subscribe();
        let event_bus = TransactionalEventBus::new(Arc::new(transport));
        let post_service = PostService::new(db.clone(), event_bus);

        let tenant_id = Uuid::new_v4();
        let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
        let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

        let denied_create = post_service
            .create_post(
                tenant_id,
                customer.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: "Customer draft".to_string(),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    excerpt: None,
                    slug: Some("customer-draft".to_string()),
                    publish: false,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await
            .expect_err("customer should not create posts");
        assert!(matches!(denied_create, BlogError::Forbidden(_)));

        let post_id = post_service
            .create_post(
                tenant_id,
                admin.clone(),
                CreatePostInput {
                    locale: "en".to_string(),
                    title: "Admin draft".to_string(),
                    body: "Body".to_string(),
                    body_format: "markdown".to_string(),
                    content_json: None,
                    excerpt: None,
                    slug: Some("admin-draft".to_string()),
                    publish: false,
                    tags: vec![],
                    category_id: None,
                    featured_image_url: None,
                    seo_title: None,
                    seo_description: None,
                    channel_slugs: None,
                    metadata: None,
                },
            )
            .await
            .expect("admin draft should be created");

        let denied_read = post_service
            .get_post(tenant_id, customer.clone(), post_id, "en")
            .await
            .expect_err("customer should not read drafts");
        assert!(matches!(denied_read, BlogError::Forbidden(_)));

        let listed = post_service
            .list_posts(
                tenant_id,
                customer,
                PostListQuery {
                    page: Some(1),
                    per_page: Some(10),
                    ..Default::default()
                },
            )
            .await
            .expect("customer listing should succeed");
        assert!(listed.items.is_empty());
        assert_eq!(listed.total, 0);
    }
}
