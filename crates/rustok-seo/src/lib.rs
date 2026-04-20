pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
pub mod services;

use async_trait::async_trait;
use rustok_core::{MigrationSource, Permission, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub use dto::{
    SeoAlternateLink, SeoDocument, SeoImageAsset, SeoLinkTag, SeoMetaInput, SeoMetaRecord,
    SeoMetaTag, SeoMetaTranslationInput, SeoMetaTranslationRecord, SeoModuleSettings, SeoOpenGraph,
    SeoPageContext, SeoPagination, SeoRedirectDecision, SeoRedirectInput, SeoRedirectMatchType,
    SeoRedirectRecord, SeoRevisionRecord, SeoRobots, SeoRobotsPreviewRecord, SeoRouteContext,
    SeoSitemapFileRecord, SeoSitemapStatusRecord, SeoStructuredDataBlock, SeoTargetKind,
    SeoTwitterCard, SeoVerification, SeoVerificationTag,
};
pub use error::{SeoError, SeoResult};
pub use graphql::{SeoMutation, SeoQuery};
pub use services::SeoService;

pub struct SeoModule;

#[async_trait]
impl RusToKModule for SeoModule {
    fn slug(&self) -> &'static str {
        "seo"
    }

    fn name(&self) -> &'static str {
        "SEO"
    }

    fn description(&self) -> &'static str {
        "SEO metadata, routing resolution, redirects, sitemaps, and robots runtime"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["content"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::SEO_READ,
            Permission::SEO_UPDATE,
            Permission::SEO_PUBLISH,
            Permission::SEO_GENERATE,
            Permission::SEO_MANAGE,
        ]
    }
}

impl MigrationSource for SeoModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
