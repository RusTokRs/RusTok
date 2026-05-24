pub mod dto;
#[cfg(feature = "server")]
pub mod service;

#[cfg(feature = "server")]
use async_trait::async_trait;
#[cfg(feature = "server")]
use rustok_core::permissions::{Action, Permission, Resource};
#[cfg(feature = "server")]
use rustok_core::{MigrationSource, RusToKModule};
#[cfg(feature = "server")]
use sea_orm_migration::MigrationTrait;

#[cfg(feature = "server")]
pub struct PageBuilderModule;

#[cfg(feature = "server")]
#[async_trait]
impl RusToKModule for PageBuilderModule {
    fn slug(&self) -> &'static str {
        "page_builder"
    }

    fn name(&self) -> &'static str {
        "Page Builder"
    }

    fn description(&self) -> &'static str {
        "Standalone FBA-first visual page builder reference module"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::new(Resource::Pages, Action::Create),
            Permission::new(Resource::Pages, Action::Read),
            Permission::new(Resource::Pages, Action::Update),
            Permission::new(Resource::Pages, Action::Delete),
            Permission::new(Resource::Pages, Action::Publish),
            Permission::new(Resource::Pages, Action::Manage),
        ]
    }
}

#[cfg(feature = "server")]
impl MigrationSource for PageBuilderModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[cfg(feature = "server")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{
        BuilderCapabilityKind, BuilderNodePropertiesInput, PublishPageBuilderInput,
        PublishPageBuilderResult,
    };

    #[test]
    fn module_metadata_is_stable() {
        let module = PageBuilderModule;

        assert_eq!(module.slug(), "page_builder");
        assert_eq!(module.name(), "Page Builder");
        assert_eq!(
            module.description(),
            "Standalone FBA-first visual page builder reference module"
        );
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn dto_contract_roundtrip_is_stable() {
        let input = PublishPageBuilderInput {
            page_id: "home".to_string(),
            revision_id: "rev-1".to_string(),
            schema_version: "grapesjs_v1".to_string(),
            project_data: serde_json::json!({ "pages": [] }),
        };
        let encoded = serde_json::to_string(&input).expect("serialize input");
        let decoded: PublishPageBuilderInput =
            serde_json::from_str(&encoded).expect("deserialize input");
        assert_eq!(decoded.page_id, "home");
        assert_eq!(decoded.schema_version, "grapesjs_v1");

        let props = BuilderNodePropertiesInput {
            page_id: "home".to_string(),
            node_id: "hero".to_string(),
            properties: serde_json::json!({ "title": "Welcome" }),
        };
        let props_json = serde_json::to_value(&props).expect("serialize props");
        assert_eq!(props_json["node_id"], "hero");

        let result = PublishPageBuilderResult {
            page_id: "home".to_string(),
            revision_id: "rev-2".to_string(),
            published: true,
        };
        assert!(result.published);
        assert_eq!(
            BuilderCapabilityKind::Publish.as_str(),
            "publish",
            "capability enum string contract must stay stable"
        );
    }
}
