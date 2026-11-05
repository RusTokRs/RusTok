use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_core::PermissionScope;

mod filters;
mod ordering;
mod related;
mod single;

pub(crate) use ordering::*;

use super::*;
use filters::{apply_post_slug_filter, apply_post_title_filter, prepare_post_list_text_filters};

impl PostService {
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

    #[instrument(skip(self, security))]
    pub async fn list_posts_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostListResponse> {
        self.list_posts_with_locale_fallback_on_channel(
            tenant_id,
            security,
            query,
            fallback_locale,
            None,
        )
        .await
    }

    /// Channel-aware list entrypoint for authenticated transports. Public
    /// restrictions apply in SQL *before* keyset LIMIT/cursor computation.
    #[instrument(skip(self, security))]
    pub async fn list_posts_with_locale_fallback_on_channel(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: PostListQuery,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> BlogResult<PostListResponse> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        enforce_scope(&security, Resource::BlogPosts, Action::List)?;
        let locale = query
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;

        let tag_filter = query.tag.clone();
        // Admin text filters are normalized once here: every later SQL builder
        // receives an already-trimmed, length-capped needle (or none at all).
        let text_filters =
            prepare_post_list_text_filters(&query, locale.as_str(), fallback_locale.as_deref())?;
        let mut select =
            blog_post::Entity::find().filter(blog_post::Column::TenantId.eq(tenant_id));

        if let Some(ref tag) = tag_filter {
            let Some(tag_id) = resolve_tag_id_for_posts(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?
            else {
                return Ok(PostListResponse::new(Vec::new(), None, query.per_page()));
            };
            select = apply_tag_filter(select, tenant_id, tag_id);
        }

        // One definition of the public corpus, shared with the public keyset list,
        // the comment target gate, the SEO scan and the Search projection: a post
        // is public when it is `Published` **and** carries its publication
        // timestamp. `published_at` is stamped by every path that enters the
        // published state, so a published row without it is damaged state, and a
        // caller without the authority to read non-public posts must not see it —
        // otherwise the customer-facing list would show a post that the public
        // list, the sitemap and the comments gate all refuse.
        let can_read_non_public = can_read_non_public_posts(&security);
        // An author with Own scope may list their own posts regardless of
        // publication status. Channel visibility still applies to public reads.
        let can_read_own = security.user_id.is_some()
            && matches!(
                security.get_scope(Resource::BlogPosts, Action::Read),
                PermissionScope::Own
            );
        if can_read_own {
            select = select.filter(
                blog_post::Column::AuthorId.eq(security.user_id.expect("user_id checked above")),
            );
        }
        if let Some(status) = query.status {
            select = select.filter(blog_post::Column::Status.eq(status_to_storage(status)));
            if status == BlogPostStatus::Published && !can_read_non_public && !can_read_own {
                select = select.filter(blog_post::Column::PublishedAt.is_not_null());
            }
        } else if !can_read_non_public && !can_read_own {
            select = select
                .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)))
                .filter(blog_post::Column::PublishedAt.is_not_null());
        }
        if !can_read_non_public
            && !can_read_own
            && matches!(query.status, Some(status) if status != BlogPostStatus::Published)
        {
            return Ok(PostListResponse::new(Vec::new(), None, query.per_page()));
        }
        if !can_read_non_public && !can_read_own {
            if !self
                .channel_is_public_visible(tenant_id, channel_slug)
                .await?
            {
                return Ok(PostListResponse::new(Vec::new(), None, query.per_page()));
            }
            select = apply_public_post_channel_filter(select, tenant_id, channel_slug);
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(blog_post::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(blog_post::Column::CategoryId.eq(category_id));
        }
        if let Some(ref needle) = text_filters.slug {
            select = apply_post_slug_filter(select, needle);
        }
        if let Some(ref needle) = text_filters.title {
            select = apply_post_title_filter(
                select,
                needle,
                &text_filters.title_locales,
                self.db.get_database_backend(),
            )?;
        }

        let (sort_by, sort_order) = query.sort();
        if let Some(raw) = query.after.as_deref() {
            let after = AdminPostCursor::decode(raw, sort_by, sort_order)?;
            select = apply_admin_post_keyset(select, after);
        }
        // Fetches one extra row to learn whether another page exists, so the list never counts rows.
        let per_page = u64::from(query.per_page());
        let select = apply_admin_post_order(select, sort_by, sort_order);
        let mut posts = select
            .limit(per_page + 1)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        let has_next_page = posts.len() as u64 > per_page;
        posts.truncate(per_page as usize);
        let next_cursor = if has_next_page {
            let last = posts.last().ok_or_else(|| {
                BlogError::invariant("Keyset page reported a next page without rows")
            })?;
            Some(
                AdminPostCursor {
                    sort_by,
                    sort_order,
                    value: admin_post_sort_value(last, sort_by),
                    id: last.id,
                }
                .encode(),
            )
        } else {
            None
        };
        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();

        let translations_map = self.load_translations_map(tenant_id, &post_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(tenant_id, &post_ids).await?;
        let tags_map = load_post_tags_map(
            &self.db,
            tenant_id,
            &post_ids,
            &locale,
            fallback_locale.as_deref(),
        )
        .await?;
        let category_ids = posts
            .iter()
            .filter_map(|post| post.category_id)
            .collect::<Vec<_>>();
        let category_names_map = self
            .load_category_names_map(
                tenant_id,
                &category_ids,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;

        let items = posts
            .into_iter()
            .filter_map(|post| {
                Self::project_post_summary(
                    tenant_id,
                    post,
                    &translations_map,
                    &tags_map,
                    &channel_slugs_map,
                    &category_names_map,
                    &locale,
                    fallback_locale.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        Ok(PostListResponse::new(items, next_cursor, query.per_page()))
    }

    /// Public channel gate. Returns `false` when the channel is unknown, inactive,
    /// or does not enable Blog; the caller then returns an empty page.
    async fn channel_is_public_visible(
        &self,
        tenant_id: Uuid,
        channel_slug: Option<&str>,
    ) -> BlogResult<bool> {
        let Some(channel_slug) = normalize_public_channel_slug(channel_slug) else {
            return Ok(true);
        };
        let channel_service = rustok_channel::ChannelService::new(self.db.clone());
        let Some(channel) = channel_service
            .get_channel_by_slug(tenant_id, channel_slug.as_str())
            .await
            .map_err(BlogError::from)?
        else {
            return Ok(false);
        };
        Ok(channel.is_active
            && channel_service
                .is_module_enabled_for_tenant(tenant_id, channel.id, "blog")
                .await
                .map_err(BlogError::from)?)
    }

    /// Builds public summaries for already-filtered posts: translations, channel
    /// slugs, tags and category names are loaded in batch for the page only.
    async fn summarize_posts(
        &self,
        tenant_id: Uuid,
        posts: Vec<blog_post::Model>,
        locale: String,
        fallback_locale: Option<String>,
    ) -> BlogResult<Vec<PostSummary>> {
        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();

        let translations_map = self.load_translations_map(tenant_id, &post_ids).await?;
        let channel_slugs_map = self.load_channel_slugs_map(tenant_id, &post_ids).await?;
        let tags_map = load_post_tags_map(
            &self.db,
            tenant_id,
            &post_ids,
            &locale,
            fallback_locale.as_deref(),
        )
        .await?;
        let category_ids = posts
            .iter()
            .filter_map(|post| post.category_id)
            .collect::<Vec<_>>();
        let category_names_map = self
            .load_category_names_map(
                tenant_id,
                &category_ids,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?;

        let items = posts
            .into_iter()
            .filter_map(|post| {
                Self::project_post_summary(
                    tenant_id,
                    post,
                    &translations_map,
                    &tags_map,
                    &channel_slugs_map,
                    &category_names_map,
                    &locale,
                    fallback_locale.as_deref(),
                )
            })
            .collect::<Vec<_>>();

        Ok(items)
    }

    /// Public keyset page of published posts, ordered `(is_pinned DESC, pinned_at DESC
    /// NULLS LAST, published_at DESC, id DESC)` — see `order_public_post_list`. Fetches
    /// one extra row to learn whether another page exists, so the list never counts rows.
    /// The returned cursor carries the full sort key, so pinned rows cannot make the
    /// listing skip or repeat entries between pages.
    #[instrument(skip(self))]
    pub async fn list_public_visible_keyset(
        &self,
        tenant_id: Uuid,
        query: PublicPostsPageQuery,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> BlogResult<PublicPostPage> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        let locale = query
            .locale
            .clone()
            .or_else(|| fallback_locale.map(str::to_string))
            .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
        let locale = normalize_locale(&locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let per_page = u64::from(query.per_page());

        if !self
            .channel_is_public_visible(tenant_id, channel_slug)
            .await?
        {
            return Ok(PublicPostPage {
                items: Vec::new(),
                next_cursor: None,
            });
        }

        // A post with no translation cannot produce a public summary. Exclude
        // such damaged rows *before* keyset LIMIT/cursor calculation; filtering
        // them only in summarize_posts can otherwise return an empty page while
        // intact posts remain beyond its cursor.
        let translated_post_ids = Query::select()
            .column(blog_post_translation::Column::PostId)
            .from(blog_post_translation::Entity)
            .to_owned();
        let mut select = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)))
            .filter(blog_post::Column::PublishedAt.is_not_null())
            .filter(blog_post::Column::Id.in_subquery(translated_post_ids));

        if let Some(ref tag) = query.tag {
            let Some(tag_id) = resolve_tag_id_for_posts(
                &self.db,
                tenant_id,
                tag,
                &locale,
                fallback_locale.as_deref(),
            )
            .await?
            else {
                return Ok(PublicPostPage {
                    items: Vec::new(),
                    next_cursor: None,
                });
            };
            select = apply_tag_filter(select, tenant_id, tag_id);
        }
        if let Some(author_id) = query.author_id {
            select = select.filter(blog_post::Column::AuthorId.eq(author_id));
        }
        if let Some(category_id) = query.category_id {
            select = select.filter(blog_post::Column::CategoryId.eq(category_id));
        }
        if let Some(after) = query.after {
            select = select.filter(public_post_after_condition(&after));
        }

        select = apply_public_post_channel_filter(select, tenant_id, channel_slug);
        let mut posts = order_public_post_list(select)
            .limit(per_page + 1)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let has_next_page = posts.len() as u64 > per_page;
        posts.truncate(per_page as usize);
        let next_cursor = if has_next_page {
            let last = posts.last().ok_or_else(|| {
                BlogError::invariant("Keyset page reported a next page without rows")
            })?;
            Some(public_post_cursor(last)?)
        } else {
            None
        };

        let items = self
            .summarize_posts(tenant_id, posts, locale, fallback_locale)
            .await?;
        Ok(PublicPostPage { items, next_cursor })
    }

    /// Keyset scan of the published corpus ordered by `id`, for bulk and sitemap jobs.
    /// Callers loop until `next_after` is `None`; no row is counted.
    ///
    /// The scan returns the SEO projection ([`PublishedSeoPost`]) and not a
    /// [`PostSummary`]: a sitemap job walks the **whole** corpus, so every
    /// projection it does not consume (tag links, category names, channel
    /// allowlists, the article body of every translation) would be paid once per
    /// page. Translations are loaded for the page in one batch and only the
    /// fields the SEO mappers use are projected.
    ///
    /// Rows that cannot be projected are skipped with a warning instead of
    /// failing the page, matching the policy of the list read path: one damaged
    /// row must not blank the sitemap of the whole tenant. A skipped row still
    /// occupies its slot in the keyset window, so the cursor keeps advancing.
    #[instrument(skip(self))]
    pub async fn scan_published_posts(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        after: Option<Uuid>,
        per_page: u32,
    ) -> BlogResult<PublishedPostScanPage> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let per_page = u64::from(per_page.clamp(1, 100));

        let mut select = blog_post::Entity::find()
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Status.eq(status_to_storage(BlogPostStatus::Published)));
        if let Some(after) = after {
            select = select.filter(blog_post::Column::Id.gt(after));
        }
        let mut posts = select
            .order_by_asc(blog_post::Column::Id)
            .limit(per_page + 1)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        let has_next_page = posts.len() as u64 > per_page;
        posts.truncate(per_page as usize);
        let next_after = if has_next_page {
            Some(
                posts
                    .last()
                    .ok_or_else(|| {
                        BlogError::invariant("Keyset scan reported a next page without rows")
                    })?
                    .id,
            )
        } else {
            None
        };

        let post_ids = posts.iter().map(|post| post.id).collect::<Vec<_>>();
        let translations_map = self.load_translations_map(tenant_id, &post_ids).await?;

        let mut items = Vec::with_capacity(posts.len());
        for post in posts {
            // `published_at` is stamped by every path that enters the published
            // state and is never reset, so a published row without it can only
            // be pre-existing corruption. Such a row has no publication date to
            // report (sitemap `lastmod`, structured data), so it is skipped
            // rather than published with an empty date.
            if post.published_at.is_none() {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    post_id = %post.id,
                    "Skipping published Blog post without a publication timestamp in SEO scan"
                );
                continue;
            }
            let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
            if translations.is_empty() {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    post_id = %post.id,
                    "Skipping Blog post without localized records in SEO scan"
                );
                continue;
            }
            let resolved =
                resolve_translation_record(&translations, &locale, fallback_locale.as_deref());
            let Some(translation) = resolved.translation else {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    post_id = %post.id,
                    "Skipping Blog post whose locale resolver returned no translation in SEO scan"
                );
                continue;
            };

            items.push(PublishedSeoPost {
                id: post.id,
                slug: post.slug.clone(),
                title: translation.title.clone(),
                featured_image_url: post.featured_image_url.clone(),
                effective_locale: resolved.effective_locale,
                available_locales: available_locales_from(&translations, |item| {
                    item.locale.as_str()
                }),
            });
        }

        Ok(PublishedPostScanPage { items, next_after })
    }

    pub(super) async fn load_category_names_map(
        &self,
        tenant_id: Uuid,
        category_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<HashMap<Uuid, String>> {
        crate::services::category_name_projection::load_category_names_map(
            &self.db,
            tenant_id,
            category_ids,
            locale,
            fallback_locale,
        )
        .await
    }

    pub(super) async fn build_post_response(
        &self,
        post: blog_post::Model,
        translations: Vec<blog_post_translation::Model>,
        channel_slugs: Vec<String>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<PostResponse> {
        let tags_map = load_post_tags_map(
            &self.db,
            post.tenant_id,
            &[post.id],
            locale,
            fallback_locale,
        )
        .await?;
        let category_name = if let Some(category_id) = post.category_id {
            self.load_category_names_map(post.tenant_id, &[category_id], locale, fallback_locale)
                .await?
                .get(&category_id)
                .cloned()
        } else {
            None
        };
        if translations.is_empty() {
            return Err(BlogError::invariant(format!(
                "Blog post {} has no localized records",
                post.id
            )));
        }
        let resolved = resolve_translation_record(&translations, locale, fallback_locale);
        let translation = resolved.translation.ok_or_else(|| {
            BlogError::invariant(format!(
                "Blog post {} locale resolver returned no translation from a non-empty set",
                post.id
            ))
        })?;
        let (content, content_plain_text) = project_stored_article(&translation.body)?;

        Ok(PostResponse {
            id: post.id,
            tenant_id: post.tenant_id,
            author_id: post.author_id,
            title: translation.title.clone(),
            slug: post.slug,
            requested_locale: locale.to_string(),
            locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            available_locales: available_locales_from(&translations, |item| item.locale.as_str()),
            content,
            content_plain_text,
            excerpt: translation.excerpt.clone(),
            status: storage_to_status(&post.status)?,
            category_id: post.category_id,
            category_name,
            tags: tags_map.get(&post.id).cloned().unwrap_or_default(),
            featured_image_url: post.featured_image_url,
            seo_title: translation.seo_title.clone(),
            seo_description: translation.seo_description.clone(),
            channel_slugs,
            metadata: scrub_reserved_metadata(post.metadata),
            comment_count: post.comment_count as i64,
            created_at: post.created_at.into(),
            updated_at: post.updated_at.into(),
            published_at: post.published_at.map(Into::into),
            scheduled_at: post.scheduled_at.map(Into::into),
            is_pinned: post.is_pinned,
            pinned_at: post.pinned_at.map(Into::into),
            version: post.version,
        })
    }

    /// Projects one post row plus its batch-loaded dependants into a
    /// [`PostSummary`].
    ///
    /// Both list read paths (`list_posts` and the public `summarize_posts`) use
    /// this one projection: a field added to `PostSummary` must be added here
    /// once, so an admin list and a public list of the same post cannot drift
    /// apart.
    ///
    /// Returns `None` for a row that cannot be projected (invalid version,
    /// missing translations, a locale that resolves to nothing, an unknown
    /// persisted status). The caller skips it and the warning names the row:
    /// one damaged row must not fail the page for every reader, while the write
    /// path still rejects creating such rows.
    fn project_post_summary(
        tenant_id: Uuid,
        post: blog_post::Model,
        translations_map: &HashMap<Uuid, Vec<blog_post_translation::Model>>,
        tags_map: &HashMap<Uuid, Vec<String>>,
        channel_slugs_map: &HashMap<Uuid, Vec<String>>,
        category_names_map: &HashMap<Uuid, String>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> Option<PostSummary> {
        if let Err(error) = Self::validate_persisted_version(&post) {
            tracing::warn!(
                tenant_id = %tenant_id,
                post_id = %post.id,
                version = post.version,
                %error,
                "Skipping Blog post with an invalid persisted version in post list"
            );
            return None;
        }
        let translations = translations_map.get(&post.id).cloned().unwrap_or_default();
        if translations.is_empty() {
            tracing::warn!(
                tenant_id = %tenant_id,
                post_id = %post.id,
                "Skipping Blog post without localized records in post list"
            );
            return None;
        }
        let resolved = resolve_translation_record(&translations, locale, fallback_locale);
        let Some(translation) = resolved.translation else {
            tracing::warn!(
                tenant_id = %tenant_id,
                post_id = %post.id,
                "Skipping Blog post whose locale resolver returned no translation in post list"
            );
            return None;
        };
        let Ok(status) = storage_to_status(&post.status) else {
            tracing::warn!(
                tenant_id = %tenant_id,
                post_id = %post.id,
                status = %post.status,
                "Skipping Blog post with an unknown persisted status in post list"
            );
            return None;
        };

        Some(PostSummary {
            id: post.id,
            title: translation.title.clone(),
            slug: post.slug.clone(),
            locale: locale.to_string(),
            effective_locale: resolved.effective_locale,
            available_locales: available_locales_from(&translations, |item| item.locale.as_str()),
            excerpt: translation.excerpt.clone(),
            status,
            author_id: post.author_id,
            author_name: None,
            category_id: post.category_id,
            category_name: post
                .category_id
                .and_then(|category_id| category_names_map.get(&category_id).cloned()),
            tags: tags_map.get(&post.id).cloned().unwrap_or_default(),
            featured_image_url: post.featured_image_url.clone(),
            channel_slugs: channel_slugs_map.get(&post.id).cloned().unwrap_or_default(),
            comment_count: post.comment_count as i64,
            published_at: post.published_at.map(Into::into),
            scheduled_at: post.scheduled_at.map(Into::into),
            is_pinned: post.is_pinned,
            pinned_at: post.pinned_at.map(Into::into),
            created_at: post.created_at.into(),
        })
    }
}
