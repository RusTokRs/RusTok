use std::collections::BTreeSet;
use std::sync::Arc;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextPrepareRequest,
};
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::{FieldError, Pos, Request, Response, ServerResult};
use rustok_api::{Permission, graphql::GraphQLError, has_effective_permission};

use crate::context::AuthContext;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ModuleAuthority {
    Read,
    Manage,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ModuleGraphqlField {
    name: &'static str,
    authority: ModuleAuthority,
}

impl ModuleGraphqlField {
    fn classify(operation_type: OperationType, field_name: &str) -> Option<Self> {
        let authority = match (operation_type, field_name) {
            (
                OperationType::Query,
                "enabledModules"
                | "moduleRegistry"
                | "tenantModules"
                | "artifactTenantLifecycle"
                | "artifactUiContributions"
                | "artifactUiActionAudit"
                | "installedModules"
                | "moduleCompositionSnapshot"
                | "marketplace"
                | "marketplaceModule"
                | "moduleOperationRecoveryPlan"
                | "failedModuleOperationRecoveryPlans"
                | "activeBuild"
                | "buildHistory",
            ) => ModuleAuthority::Read,
            (OperationType::Query, "marketplaceRegistryFreshness") => ModuleAuthority::Manage,
            (OperationType::Mutation, "executeArtifactUiAction") => ModuleAuthority::Read,
            (
                OperationType::Mutation,
                "installModule"
                | "uninstallModule"
                | "upgradeModule"
                | "toggleModule"
                | "setArtifactTenantEnabled"
                | "activateTenantArtifact"
                | "deactivateTenantArtifact"
                | "uninstallTenantArtifact"
                | "rollbackTenantArtifact"
                | "retryFailedModuleOperationPostHook"
                | "compensateFailedModuleOperation"
                | "updateModuleSettings",
            ) => ModuleAuthority::Manage,
            (OperationType::Subscription, "buildProgress") => ModuleAuthority::Read,
            _ => return None,
        };

        Some(Self {
            name: match field_name {
                "enabledModules" => "enabledModules",
                "moduleRegistry" => "moduleRegistry",
                "tenantModules" => "tenantModules",
                "artifactTenantLifecycle" => "artifactTenantLifecycle",
                "artifactUiContributions" => "artifactUiContributions",
                "artifactUiActionAudit" => "artifactUiActionAudit",
                "installedModules" => "installedModules",
                "moduleCompositionSnapshot" => "moduleCompositionSnapshot",
                "marketplace" => "marketplace",
                "marketplaceModule" => "marketplaceModule",
                "marketplaceRegistryFreshness" => "marketplaceRegistryFreshness",
                "moduleOperationRecoveryPlan" => "moduleOperationRecoveryPlan",
                "failedModuleOperationRecoveryPlans" => "failedModuleOperationRecoveryPlans",
                "activeBuild" => "activeBuild",
                "buildHistory" => "buildHistory",
                "executeArtifactUiAction" => "executeArtifactUiAction",
                "installModule" => "installModule",
                "uninstallModule" => "uninstallModule",
                "upgradeModule" => "upgradeModule",
                "toggleModule" => "toggleModule",
                "setArtifactTenantEnabled" => "setArtifactTenantEnabled",
                "activateTenantArtifact" => "activateTenantArtifact",
                "deactivateTenantArtifact" => "deactivateTenantArtifact",
                "uninstallTenantArtifact" => "uninstallTenantArtifact",
                "rollbackTenantArtifact" => "rollbackTenantArtifact",
                "retryFailedModuleOperationPostHook" => "retryFailedModuleOperationPostHook",
                "compensateFailedModuleOperation" => "compensateFailedModuleOperation",
                "updateModuleSettings" => "updateModuleSettings",
                "buildProgress" => "buildProgress",
                _ => return None,
            },
            authority,
        })
    }

    fn allowed(self, permissions: &[Permission]) -> bool {
        match self.authority {
            ModuleAuthority::Read => {
                has_effective_permission(permissions, &Permission::MODULES_READ)
                    || has_effective_permission(permissions, &Permission::MODULES_LIST)
                    || has_effective_permission(permissions, &Permission::MODULES_MANAGE)
            }
            ModuleAuthority::Manage => {
                has_effective_permission(permissions, &Permission::MODULES_MANAGE)
            }
        }
    }

    fn permission_hint(self) -> &'static str {
        match self.authority {
            ModuleAuthority::Read => "modules:read, modules:list, or modules:manage",
            ModuleAuthority::Manage => "modules:manage",
        }
    }
}

#[derive(Clone, Debug)]
struct ModuleGraphqlDocumentPolicy(Vec<ModuleGraphqlField>);

fn collect_fields(
    operation_type: OperationType,
    selection_set: &SelectionSet,
    document: &ExecutableDocument,
    fields: &mut BTreeSet<ModuleGraphqlField>,
) {
    for selection in &selection_set.items {
        match &selection.node {
            Selection::Field(field) => {
                if let Some(field) =
                    ModuleGraphqlField::classify(operation_type, field.node.name.node.as_str())
                {
                    fields.insert(field);
                }
            }
            Selection::FragmentSpread(fragment) => {
                if let Some(definition) = document.fragments.get(&fragment.node.fragment_name.node)
                {
                    collect_fields(
                        operation_type,
                        &definition.node.selection_set.node,
                        document,
                        fields,
                    );
                }
            }
            Selection::InlineFragment(fragment) => collect_fields(
                operation_type,
                &fragment.node.selection_set.node,
                document,
                fields,
            ),
        }
    }
}

fn classify_document(request: &mut Request) -> ServerResult<()> {
    if request.query.trim().is_empty() {
        return Ok(());
    }

    let document = request.parsed_query()?;
    let mut fields = BTreeSet::new();
    for (_, operation) in document.operations.iter() {
        collect_fields(
            operation.node.ty,
            &operation.node.selection_set.node,
            document,
            &mut fields,
        );
    }
    if !fields.is_empty() {
        request
            .data
            .insert(ModuleGraphqlDocumentPolicy(fields.into_iter().collect()));
    }
    Ok(())
}

#[derive(Default)]
pub struct GraphqlModuleSecurityPolicy;

impl ExtensionFactory for GraphqlModuleSecurityPolicy {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(GraphqlModuleSecurityPolicyExtension)
    }
}

struct GraphqlModuleSecurityPolicyExtension;

#[async_trait::async_trait]
impl Extension for GraphqlModuleSecurityPolicyExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let mut request = next.run(ctx, request).await?;
        classify_document(&mut request)?;
        Ok(request)
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        if let Some(policy) = ctx.data_opt::<ModuleGraphqlDocumentPolicy>() {
            let auth = ctx.data_opt::<AuthContext>();
            let denied = policy
                .0
                .iter()
                .copied()
                .filter(|field| auth.is_none_or(|auth| !field.allowed(&auth.permissions)))
                .collect::<Vec<_>>();

            if !denied.is_empty() {
                tracing::warn!(
                    denied_fields = ?denied.iter().map(|field| field.name).collect::<Vec<_>>(),
                    operation_name = ?operation_name,
                    "Rejected module GraphQL document before resolver execution"
                );
                let required = denied
                    .iter()
                    .map(|field| format!("{} -> {}", field.name, field.permission_hint()))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Response::from_errors(vec![
                    <FieldError as GraphQLError>::permission_denied(&format!(
                        "Forbidden module GraphQL operation. Required permissions: {required}"
                    ))
                    .into_server_error(Pos::default()),
                ]);
            }
        }

        next.run(ctx, operation_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleAuthority, ModuleGraphqlDocumentPolicy, ModuleGraphqlField, classify_document,
    };
    use async_graphql::Request;
    use rustok_api::Permission;

    #[test]
    fn classifies_module_fields_inside_fragments_and_subscriptions() {
        let mut request = Request::new(
            r#"
                query ModuleState { ...ModuleFields }
                fragment ModuleFields on Query {
                    enabledModules
                    moduleRegistry { moduleSlug }
                    artifactTenantLifecycle(
                        installationId: "00000000-0000-0000-0000-000000000000"
                    ) { expectedRevision }
                    artifactUiContributions(installationId: "00000000-0000-0000-0000-000000000000") { id }
                    artifactUiActionAudit(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        contributionId: "apply"
                    ) { executionId }
                    moduleCompositionSnapshot { revision }
                }
                subscription BuildState { buildProgress { buildId } }
            "#,
        );

        classify_document(&mut request).expect("document should parse");
        let policy = request
            .data
            .get(&std::any::TypeId::of::<ModuleGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<ModuleGraphqlDocumentPolicy>())
            .expect("module policy should be attached");

        assert!(policy.0.iter().any(
            |field| field.name == "enabledModules" && field.authority == ModuleAuthority::Read
        ));
        assert!(policy.0.iter().any(
            |field| field.name == "moduleRegistry" && field.authority == ModuleAuthority::Read
        ));
        assert!(
            policy
                .0
                .iter()
                .any(|field| field.name == "artifactTenantLifecycle"
                    && field.authority == ModuleAuthority::Read)
        );
        assert!(
            policy
                .0
                .iter()
                .any(|field| field.name == "artifactUiContributions"
                    && field.authority == ModuleAuthority::Read)
        );
        assert!(
            policy
                .0
                .iter()
                .any(|field| field.name == "artifactUiActionAudit"
                    && field.authority == ModuleAuthority::Read)
        );
        assert!(
            policy
                .0
                .iter()
                .any(|field| field.name == "moduleCompositionSnapshot"
                    && field.authority == ModuleAuthority::Read)
        );
        assert!(
            policy
                .0
                .iter()
                .any(|field| field.name == "buildProgress"
                    && field.authority == ModuleAuthority::Read)
        );
    }

    #[test]
    fn classifies_module_mutations_as_manage() {
        let mut request = Request::new(
            r#"mutation ChangeModules { toggleModule(moduleSlug: "blog", enabled: true) { moduleSlug } }"#,
        );

        classify_document(&mut request).expect("document should parse");
        let policy = request
            .data
            .get(&std::any::TypeId::of::<ModuleGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<ModuleGraphqlDocumentPolicy>())
            .expect("module policy should be attached");

        assert_eq!(policy.0.len(), 1);
        assert_eq!(policy.0[0].authority, ModuleAuthority::Manage);
    }

    #[test]
    fn classifies_marketplace_registry_freshness_as_manage() {
        let mut request = Request::new(
            r#"
                query MarketplaceRegistryFreshness {
                    marketplaceRegistryFreshness { registryId }
                }
            "#,
        );

        classify_document(&mut request).expect("document should parse");
        let policy = request
            .data
            .get(&std::any::TypeId::of::<ModuleGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<ModuleGraphqlDocumentPolicy>())
            .expect("module policy should be attached");

        assert_eq!(policy.0.len(), 1);
        assert_eq!(policy.0[0].name, "marketplaceRegistryFreshness");
        assert_eq!(policy.0[0].authority, ModuleAuthority::Manage);
    }

    #[test]
    fn classifies_artifact_ui_actions_as_read_before_dynamic_rbac() {
        let mut request = Request::new(
            r#"
                mutation ExecuteArtifactUiAction {
                    executeArtifactUiAction(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        contributionId: "apply"
                        input: {}
                        idempotencyKey: "00000000-0000-0000-0000-000000000000"
                    )
                }
            "#,
        );

        classify_document(&mut request).expect("document should parse");
        let policy = request
            .data
            .get(&std::any::TypeId::of::<ModuleGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<ModuleGraphqlDocumentPolicy>())
            .expect("module policy should be attached");

        assert_eq!(policy.0.len(), 1);
        assert_eq!(policy.0[0].name, "executeArtifactUiAction");
        assert_eq!(policy.0[0].authority, ModuleAuthority::Read);
    }

    #[test]
    fn classifies_tenant_artifact_lifecycle_mutations_as_manage() {
        let mut request = Request::new(
            r#"
                mutation TenantArtifactLifecycle {
                    setArtifactTenantEnabled(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        enabled: true
                        expectedRevision: 1
                        reason: "enable"
                        idempotencyKey: "00000000-0000-0000-0000-000000000001"
                    ) { revision }
                    activateTenantArtifact(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        expectedRevision: 1
                        reason: "activate"
                        idempotencyKey: "11111111-1111-1111-1111-111111111111"
                    ) { operationId }
                    deactivateTenantArtifact(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        expectedRevision: 2
                        reason: "deactivate"
                        idempotencyKey: "22222222-2222-2222-2222-222222222222"
                    ) { operationId }
                    uninstallTenantArtifact(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        expectedRevision: 3
                        reason: "uninstall"
                        idempotencyKey: "33333333-3333-3333-3333-333333333333"
                    ) { operationId }
                    rollbackTenantArtifact(
                        installationId: "00000000-0000-0000-0000-000000000000"
                        expectedRevision: 4
                        reason: "rollback"
                        idempotencyKey: "44444444-4444-4444-4444-444444444444"
                        targetCapabilityGrantRevision: 1
                        migrationRollbackMode: REVERSIBLE
                    ) { targetInstallationId }
                }
            "#,
        );

        classify_document(&mut request).expect("document should parse");
        let policy = request
            .data
            .get(&std::any::TypeId::of::<ModuleGraphqlDocumentPolicy>())
            .and_then(|value| value.downcast_ref::<ModuleGraphqlDocumentPolicy>())
            .expect("module policy should be attached");

        assert_eq!(policy.0.len(), 5);
        assert!(policy.0.iter().all(|field| {
            field.authority == ModuleAuthority::Manage
                && matches!(
                    field.name,
                    "setArtifactTenantEnabled"
                        | "activateTenantArtifact"
                        | "deactivateTenantArtifact"
                        | "uninstallTenantArtifact"
                        | "rollbackTenantArtifact"
                )
        }));
    }

    #[test]
    fn module_manage_permission_includes_module_reads() {
        assert!(
            ModuleGraphqlField {
                name: "artifactUiContributions",
                authority: ModuleAuthority::Read,
            }
            .allowed(&[Permission::MODULES_MANAGE])
        );
    }
}
