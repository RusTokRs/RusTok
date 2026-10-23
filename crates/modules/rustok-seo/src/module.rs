/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    HealthStatus, MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleKind, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

use crate::migrations;
use crate::services;

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

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
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

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let event_bus = ctx
            .extensions
            .get::<rustok_outbox::TransactionalEventBus>()
            .cloned()
            .unwrap_or_else(|| {
                let transport: std::sync::Arc<dyn rustok_core::events::EventTransport> =
                    std::sync::Arc::new(rustok_outbox::OutboxTransport::new(ctx.db.clone()));
                rustok_outbox::TransactionalEventBus::new(transport)
            });
        let seo_registry = ctx
            .extensions
            .get::<rustok_seo_targets::SeoTargetRegistry>()
            .cloned()
            .unwrap_or_default();

        registry.register(services::SeoCanonicalUrlRedirectHandler::new(
            ctx.db.clone(),
            event_bus,
            std::sync::Arc::new(seo_registry),
        ));
    }
}

impl MigrationSource for SeoModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seo_module_metadata() {
        let module = SeoModule;
        assert_eq!(module.slug(), "seo");
        assert_eq!(module.name(), "SEO");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert_eq!(module.dependencies(), &["content"]);
        assert!(!module.permissions().is_empty());
        assert!(!module.migrations().is_empty());
    }

    #[tokio::test]
    async fn test_seo_module_health() {
        let module = SeoModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }
}
