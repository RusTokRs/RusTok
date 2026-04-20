use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_core::{Permission, SecurityContext, UserRole};

use crate::dto::{SeoAlternateLink, SeoOpenGraph, SeoTargetKind};
use crate::{SeoError, SeoResult};

use super::robots::image_assets_from_optional_url;
use super::routing::locale_prefixed_path;
use super::{SeoService, TargetState};

impl SeoService {
    pub(super) async fn load_target_state(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        locale: &str,
    ) -> SeoResult<Option<TargetState>> {
        match target_kind {
            SeoTargetKind::Page => self.load_page_by_id(tenant, locale, target_id).await,
            SeoTargetKind::Product => self.load_product_by_id(tenant, locale, target_id).await,
            SeoTargetKind::BlogPost => self.load_blog_post_by_id(tenant, locale, target_id).await,
            SeoTargetKind::ForumCategory => {
                self.load_forum_category_by_id(tenant, locale, target_id)
                    .await
            }
            SeoTargetKind::ForumTopic => {
                self.load_forum_topic_by_id(tenant, locale, target_id).await
            }
        }
    }

    pub(super) async fn load_route_target_state(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetKind,
        target_id: Uuid,
        locale: &str,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<TargetState>> {
        match target_kind {
            SeoTargetKind::ForumTopic => {
                self.load_public_forum_topic_by_id(tenant, locale, target_id, channel_slug)
                    .await
            }
            _ => {
                self.load_target_state(tenant, target_kind, target_id, locale)
                    .await
            }
        }
    }

    pub(super) async fn load_page_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        page_id: Uuid,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_pages::PageService::new(self.db.clone(), self.event_bus.clone());
        let page = match service
            .get_with_locale_fallback(
                tenant.id,
                public_pages_security(),
                page_id,
                locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(page) => page,
            Err(_) => return Ok(None),
        };
        Ok(Some(map_page_state(page)))
    }

    pub(super) async fn load_page_by_slug(
        &self,
        tenant: &TenantContext,
        locale: &str,
        slug: &str,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_pages::PageService::new(self.db.clone(), self.event_bus.clone());
        let page = service
            .get_by_slug_with_locale_fallback(
                tenant.id,
                public_pages_security(),
                locale,
                slug,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| SeoError::validation(err.to_string()))?;
        Ok(page.map(map_page_state))
    }

    pub(super) async fn load_blog_post_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        post_id: Uuid,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_blog::PostService::new(self.db.clone(), self.event_bus.clone());
        let post = match service
            .get_post_with_locale_fallback(
                tenant.id,
                public_blog_security(),
                post_id,
                locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(post) => post,
            Err(_) => return Ok(None),
        };
        self.map_blog_state(post).await.map(Some)
    }

    pub(super) async fn load_blog_post_by_slug(
        &self,
        tenant: &TenantContext,
        locale: &str,
        slug: &str,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_blog::PostService::new(self.db.clone(), self.event_bus.clone());
        let post = service
            .get_post_by_slug_with_locale_fallback(
                tenant.id,
                public_blog_security(),
                locale,
                slug,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|err| SeoError::validation(err.to_string()))?;
        match post {
            Some(post) => self.map_blog_state(post).await.map(Some),
            None => Ok(None),
        }
    }

    pub(super) async fn load_product_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        product_id: Uuid,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_product::CatalogService::new(self.db.clone(), self.event_bus.clone());
        let product = match service
            .get_product_with_locale_fallback(
                tenant.id,
                product_id,
                locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(product) => product,
            Err(_) => return Ok(None),
        };
        if product.status != rustok_commerce_foundation::entities::product::ProductStatus::Active
            || product.published_at.is_none()
        {
            return Ok(None);
        }
        Ok(Some(map_product_state(
            product,
            locale,
            tenant.default_locale.as_str(),
        )))
    }

    pub(super) async fn load_product_by_handle(
        &self,
        tenant: &TenantContext,
        locale: &str,
        handle: &str,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_product::CatalogService::new(self.db.clone(), self.event_bus.clone());
        let product = service
            .get_published_product_by_handle_with_locale_fallback(
                tenant.id,
                handle,
                locale,
                Some(tenant.default_locale.as_str()),
                None,
            )
            .await
            .map_err(|err| SeoError::validation(err.to_string()))?;
        Ok(product
            .map(|product| map_product_state(product, locale, tenant.default_locale.as_str())))
    }

    pub(super) async fn load_forum_category_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        category_id: Uuid,
    ) -> SeoResult<Option<TargetState>> {
        let service = rustok_forum::CategoryService::new(self.db.clone());
        let category = match service
            .get_with_locale_fallback(
                tenant.id,
                SecurityContext::system(),
                category_id,
                locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(category) => category,
            Err(rustok_forum::ForumError::CategoryNotFound(_)) => return Ok(None),
            Err(err) => return Err(SeoError::validation(err.to_string())),
        };
        Ok(Some(map_forum_category_state(category)))
    }

    pub(super) async fn load_forum_topic_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        topic_id: Uuid,
    ) -> SeoResult<Option<TargetState>> {
        let Some(topic) = self
            .load_forum_topic_response_by_id(tenant, locale, topic_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(map_forum_topic_state(topic)))
    }

    pub(super) async fn load_public_forum_topic_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        topic_id: Uuid,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<TargetState>> {
        let Some(topic) = self
            .load_forum_topic_response_by_id(tenant, locale, topic_id)
            .await?
        else {
            return Ok(None);
        };
        if topic.status != rustok_forum::constants::topic_status::OPEN
            || !is_public_forum_topic_visible(topic.channel_slugs.as_slice(), channel_slug)
        {
            return Ok(None);
        }
        Ok(Some(map_forum_topic_state(topic)))
    }

    async fn load_forum_topic_response_by_id(
        &self,
        tenant: &TenantContext,
        locale: &str,
        topic_id: Uuid,
    ) -> SeoResult<Option<rustok_forum::TopicResponse>> {
        let service = rustok_forum::TopicService::new(self.db.clone(), self.event_bus.clone());
        match service
            .get_with_locale_fallback(
                tenant.id,
                SecurityContext::system(),
                topic_id,
                locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(topic) => Ok(Some(topic)),
            Err(rustok_forum::ForumError::TopicNotFound(_)) => Ok(None),
            Err(err) => Err(SeoError::validation(err.to_string())),
        }
    }

    async fn map_blog_state(&self, post: rustok_blog::PostResponse) -> SeoResult<TargetState> {
        let translations = rustok_blog::entities::blog_post_translation::Entity::find()
            .filter(rustok_blog::entities::blog_post_translation::Column::PostId.eq(post.id))
            .all(&self.db)
            .await?;
        let locales =
            rustok_content::available_locales_from(&translations, |item| item.locale.as_str());
        let alternates = locales
            .iter()
            .map(|locale| SeoAlternateLink {
                locale: locale.clone(),
                href: locale_prefixed_path(
                    locale.as_str(),
                    format!("/modules/blog?slug={}", post.slug).as_str(),
                ),
                x_default: false,
            })
            .collect::<Vec<_>>();

        Ok(TargetState {
            target_kind: SeoTargetKind::BlogPost,
            target_id: post.id,
            requested_locale: Some(post.requested_locale.clone()),
            effective_locale: post.effective_locale.clone(),
            title: post.seo_title.clone().unwrap_or_else(|| post.title.clone()),
            description: post
                .seo_description
                .clone()
                .or(post.excerpt.clone())
                .or_else(|| summarize_text(post.body.as_str())),
            canonical_path: format!("/modules/blog?slug={}", post.slug),
            alternates,
            open_graph: SeoOpenGraph {
                title: post.seo_title.clone().or(Some(post.title.clone())),
                description: post.seo_description.clone().or(post.excerpt.clone()),
                kind: Some("article".to_string()),
                site_name: None,
                url: None,
                locale: None,
                images: image_assets_from_optional_url(post.featured_image_url.clone()),
            },
            structured_data: json!({
                "@context": "https://schema.org",
                "@type": "BlogPosting",
                "headline": post.title,
                "description": post.excerpt,
                "datePublished": post.published_at.map(|value| value.to_rfc3339()),
                "dateModified": post.updated_at.to_rfc3339(),
                "inLanguage": post.effective_locale,
                "image": post.featured_image_url,
            }),
            fallback_source: "blog_post",
        })
    }
}

pub(super) fn public_pages_security() -> SecurityContext {
    SecurityContext::from_permissions(
        UserRole::Customer,
        None,
        [Permission::PAGES_READ, Permission::PAGES_LIST],
    )
}

pub(super) fn public_blog_security() -> SecurityContext {
    SecurityContext::from_permissions(
        UserRole::Customer,
        None,
        [Permission::BLOG_POSTS_READ, Permission::BLOG_POSTS_LIST],
    )
}

pub(super) fn summarize_text(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(rustok_core::truncate(normalized.as_str(), 180))
    }
}

fn is_public_forum_topic_visible(channel_slugs: &[String], channel_slug: Option<&str>) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
    else {
        return false;
    };

    channel_slugs.iter().any(|item| item == &channel_slug)
}

pub(super) fn map_page_state(page: rustok_pages::PageResponse) -> TargetState {
    let translation = page
        .translation
        .clone()
        .or_else(|| page.translations.first().cloned());
    let slug = translation
        .as_ref()
        .and_then(|item| item.slug.clone())
        .unwrap_or_default();
    let effective_locale = page
        .effective_locale
        .clone()
        .or(page.requested_locale.clone())
        .or_else(|| page.translations.first().map(|item| item.locale.clone()))
        .unwrap_or_else(|| "en".to_string());
    let title = translation
        .as_ref()
        .and_then(|item| item.meta_title.clone())
        .or_else(|| translation.as_ref().and_then(|item| item.title.clone()))
        .unwrap_or_else(|| "Untitled page".to_string());
    let description = translation
        .as_ref()
        .and_then(|item| item.meta_description.clone())
        .or_else(|| {
            page.body
                .as_ref()
                .and_then(|body| summarize_text(body.content.as_str()))
        });
    let alternates = page
        .translations
        .iter()
        .filter_map(|item| {
            item.slug.as_ref().map(|slug| SeoAlternateLink {
                locale: item.locale.clone(),
                href: locale_prefixed_path(
                    item.locale.as_str(),
                    format!("/modules/pages?slug={slug}").as_str(),
                ),
                x_default: false,
            })
        })
        .collect::<Vec<_>>();

    TargetState {
        target_kind: SeoTargetKind::Page,
        target_id: page.id,
        requested_locale: page.requested_locale,
        effective_locale: effective_locale.clone(),
        title: title.clone(),
        description: description.clone(),
        canonical_path: format!("/modules/pages?slug={slug}"),
        alternates,
        open_graph: SeoOpenGraph {
            title: Some(title.clone()),
            description: description.clone(),
            kind: Some("website".to_string()),
            site_name: None,
            url: None,
            locale: None,
            images: Vec::new(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "WebPage",
            "name": title,
            "description": description,
            "inLanguage": effective_locale,
        }),
        fallback_source: "page",
    }
}

pub(super) fn map_product_state(
    product: rustok_commerce_foundation::dto::ProductResponse,
    locale: &str,
    default_locale: &str,
) -> TargetState {
    let (translation, effective_locale) = resolve_product_translation(
        product.translations.as_slice(),
        locale,
        Some(default_locale),
    );
    let translation = translation.cloned().unwrap_or_else(|| {
        rustok_commerce_foundation::dto::ProductTranslationResponse {
            locale: default_locale.to_string(),
            title: "Untitled product".to_string(),
            handle: String::new(),
            description: None,
            meta_title: None,
            meta_description: None,
        }
    });
    let title = translation
        .meta_title
        .clone()
        .unwrap_or_else(|| translation.title.clone());
    let description = translation
        .meta_description
        .clone()
        .or_else(|| translation.description.clone())
        .or_else(|| summarize_text(translation.title.as_str()));
    let image_url = product.images.first().map(|image| image.url.clone());
    let alternates = product
        .translations
        .iter()
        .map(|item| SeoAlternateLink {
            locale: item.locale.clone(),
            href: locale_prefixed_path(
                item.locale.as_str(),
                format!("/modules/product?handle={}", item.handle).as_str(),
            ),
            x_default: false,
        })
        .collect::<Vec<_>>();

    TargetState {
        target_kind: SeoTargetKind::Product,
        target_id: product.id,
        requested_locale: Some(locale.to_string()),
        effective_locale,
        title: title.clone(),
        description: description.clone(),
        canonical_path: format!("/modules/product?handle={}", translation.handle),
        alternates,
        open_graph: SeoOpenGraph {
            title: Some(title.clone()),
            description: description.clone(),
            kind: Some("product".to_string()),
            site_name: None,
            url: None,
            locale: None,
            images: image_assets_from_optional_url(image_url.clone()),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "Product",
            "name": translation.title,
            "description": translation.description,
            "image": image_url,
            "inLanguage": locale,
        }),
        fallback_source: "product",
    }
}

pub(super) fn map_forum_category_state(category: rustok_forum::CategoryResponse) -> TargetState {
    let alternates = category
        .available_locales
        .iter()
        .map(|locale| SeoAlternateLink {
            locale: locale.clone(),
            href: locale_prefixed_path(
                locale.as_str(),
                format!("/modules/forum?category={}", category.id).as_str(),
            ),
            x_default: false,
        })
        .collect::<Vec<_>>();
    let description = category
        .description
        .clone()
        .or_else(|| summarize_text(category.name.as_str()));

    TargetState {
        target_kind: SeoTargetKind::ForumCategory,
        target_id: category.id,
        requested_locale: Some(category.requested_locale.clone()),
        effective_locale: category.effective_locale.clone(),
        title: category.name.clone(),
        description: description.clone(),
        canonical_path: format!("/modules/forum?category={}", category.id),
        alternates,
        open_graph: SeoOpenGraph {
            title: Some(category.name.clone()),
            description: description.clone(),
            kind: Some("website".to_string()),
            site_name: None,
            url: None,
            locale: None,
            images: Vec::new(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "CollectionPage",
            "name": category.name,
            "description": description,
            "inLanguage": category.effective_locale,
            "identifier": category.id,
        }),
        fallback_source: "forum_category",
    }
}

pub(super) fn map_forum_topic_state(topic: rustok_forum::TopicResponse) -> TargetState {
    let alternates = topic
        .available_locales
        .iter()
        .map(|locale| SeoAlternateLink {
            locale: locale.clone(),
            href: locale_prefixed_path(
                locale.as_str(),
                format!(
                    "/modules/forum?category={}&topic={}",
                    topic.category_id, topic.id
                )
                .as_str(),
            ),
            x_default: false,
        })
        .collect::<Vec<_>>();
    let description = summarize_text(topic.body.as_str());

    TargetState {
        target_kind: SeoTargetKind::ForumTopic,
        target_id: topic.id,
        requested_locale: Some(topic.requested_locale.clone()),
        effective_locale: topic.effective_locale.clone(),
        title: topic.title.clone(),
        description: description.clone(),
        canonical_path: format!(
            "/modules/forum?category={}&topic={}",
            topic.category_id, topic.id
        ),
        alternates,
        open_graph: SeoOpenGraph {
            title: Some(topic.title.clone()),
            description: description.clone(),
            kind: Some("article".to_string()),
            site_name: None,
            url: None,
            locale: None,
            images: Vec::new(),
        },
        structured_data: json!({
            "@context": "https://schema.org",
            "@type": "DiscussionForumPosting",
            "headline": topic.title,
            "articleBody": topic.body,
            "keywords": topic.tags,
            "inLanguage": topic.effective_locale,
            "identifier": topic.id,
        }),
        fallback_source: "forum_topic",
    }
}

fn resolve_product_translation<'a>(
    items: &'a [rustok_commerce_foundation::dto::ProductTranslationResponse],
    requested: &str,
    fallback: Option<&str>,
) -> (
    Option<&'a rustok_commerce_foundation::dto::ProductTranslationResponse>,
    String,
) {
    let candidates =
        rustok_core::build_locale_candidates([Some(requested), fallback, Some("en")], true);
    for candidate in candidates {
        if let Some(item) = items.iter().find(|item| {
            rustok_core::normalize_locale_tag(item.locale.as_str())
                .is_some_and(|locale| locale == candidate)
        }) {
            return (Some(item), candidate);
        }
    }
    (
        items.first(),
        items
            .first()
            .and_then(|item| rustok_core::normalize_locale_tag(item.locale.as_str()))
            .unwrap_or_else(|| requested.to_string()),
    )
}
