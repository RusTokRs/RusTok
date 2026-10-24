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
use rustok_core::module::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleKind, ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

/// Calendar & Appointment Scheduling capability module for RusToK.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CalendarModule;

impl CalendarModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for CalendarModule {
    fn slug(&self) -> &'static str {
        "calendar"
    }

    fn name(&self) -> &'static str {
        "Calendar & Appointment Scheduling"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn description(&self) -> &'static str {
        "Calendar & Appointment Scheduling capability module for RusToK"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        // Standard RBAC permissions for calendar.
        // Declare registered permissions here once corresponding Resource
        // variants are added to `rustok_api::permissions::Resource`.
        Vec::new()
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        rustok_core::search_projection::register_search_projection_source(
            extensions,
            crate::integrations::search::CalendarSearchProjectionSourceFactory,
        )?;
        Ok(())
    }

    fn register_event_listeners(
        &self,
        _registry: &mut ModuleEventListenerRegistry,
        _ctx: &ModuleEventListenerContext<'_>,
    ) {
        // Register event listeners for cross-module integration or outbox event handling:
    }
}

impl MigrationSource for CalendarModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_metadata() {
        let module = CalendarModule;
        assert_eq!(module.slug(), "calendar");
        assert_eq!(module.name(), "Calendar & Appointment Scheduling");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.kind(), ModuleKind::Optional);
    }

    #[test]
    fn test_module_permissions_contract() {
        let module = CalendarModule;
        let _perms = module.permissions();
    }
}
