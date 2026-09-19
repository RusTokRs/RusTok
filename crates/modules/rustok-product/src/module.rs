use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

use crate::{migrations, seo_targets};

/// Typed marker proving that `ProductModule` participated in runtime extension registration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductRuntimeSelected;

pub struct ProductModule;

#[async_trait]
impl RusToKModule for ProductModule {
    fn slug(&self) -> &'static str { "product" }

    fn name(&self) -> &'static str { "Product" }

    fn description(&self) -> &'static str {
        "Product catalog, variants, translations, options, and publication lifecycle"
    }

    fn version(&self) -> &'static str { env!("CARGO_PKG_VERSION") }

    fn dependencies(&self) -> &[&'static str] { &["taxonomy"] }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PRODUCTS_CREATE,
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_UPDATE,
            Permission::PRODUCTS_DELETE,
            Permission::PRODUCTS_LIST,
            Permission::PRODUCTS_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_seo_target_provider(extensions, seo_targets::ProductSeoTargetProvider).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "product SEO target registration failed: {error}"
                ))
            },
        )?;
        extensions.insert(ProductRuntimeSelected);
        Ok(())
    }
}

impl MigrationSource for ProductModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
