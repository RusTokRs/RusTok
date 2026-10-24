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
use rustok_core::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaxModule;

impl TaxModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for TaxModule {
    fn slug(&self) -> &'static str {
        "tax"
    }

    fn name(&self) -> &'static str {
        "Tax"
    }

    fn description(&self) -> &'static str {
        "Tax domain foundation, tax-line calculation contract, and provider seam"
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

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

impl MigrationSource for TaxModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tax_module_metadata() {
        let module = TaxModule::new();
        assert_eq!(module.slug(), "tax");
        assert_eq!(module.name(), "Tax");
        assert_eq!(module.kind(), ModuleKind::Optional);
        assert!(module.dependencies().is_empty());
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert!(module.migrations().is_empty());
    }

    #[tokio::test]
    async fn test_tax_module_health() {
        let module = TaxModule;
        assert_eq!(module.health().await, HealthStatus::Healthy);
    }
}
