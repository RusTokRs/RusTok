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
use rustok_core::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

/// Geographical jurisdiction boundaries, country-to-region mapping, and tax policy rules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RegionModule;

impl RegionModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for RegionModule {
    fn slug(&self) -> &'static str {
        "region"
    }

    fn name(&self) -> &'static str {
        "Region"
    }

    fn description(&self) -> &'static str {
        "Default region submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::REGIONS_CREATE,
            Permission::REGIONS_READ,
            Permission::REGIONS_UPDATE,
            Permission::REGIONS_DELETE,
            Permission::REGIONS_LIST,
            Permission::REGIONS_MANAGE,
        ]
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for RegionModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_module_metadata() {
        let module = RegionModule;
        assert_eq!(module.slug(), "region");
        assert_eq!(module.name(), "Region");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert!(module.dependencies().is_empty());
        assert!(!module.permissions().is_empty());
        assert!(!module.migrations().is_empty());
    }

    #[tokio::test]
    async fn test_region_module_health() {
        let module = RegionModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }
}
