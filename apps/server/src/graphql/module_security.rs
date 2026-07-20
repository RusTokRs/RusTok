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
                "moduleRegistry"
                | "tenantModules"
                | "installedModules"
                | "marketplace"
                | "marketplaceModule"
                | "moduleOperationRecoveryPlan"
                | "failedModuleOperationRecoveryPlans"
                | "activeBuild"
                | "buildHistory"
                | "activeRelease"
                | "releaseHistory",
            ) => ModuleAuthority::Read,
            (
                OperationType::Mutation,
                "installModule"
                | "uninstallModule"
                | "upgradeModule"
                | "rollbackBuild"
                | "toggleModule"
                | "retryFailedModuleOperationPostHook"
                | "compensateFailedModuleOperation"
                | "updateModuleSettings",
            ) => ModuleAuthority::Manage,
            (OperationType::Subscription, "buildProgress") => ModuleAuthority::Read,
            _ => return None,
        };

        Some(Self {
            name: match field_name {
                "moduleRegistry" => "moduleRegistry",
                "tenantModules" => "tenantModules",
                "installedModules" => "installedModules",
                "marketplace" => "marketplace",
                "marketplaceModule" => "marketplaceModule",
                "moduleOperationRecoveryPlan" => "moduleOperationRecoveryPlan",
                "failedModuleOperationRecoveryPlans" => "failedModuleOperationRecoveryPlans",
                "activeBuild" => "activeBuild",
                "buildHistory" => "buildHistory",
                "activeRelease" => "activeRelease",
                "releaseHistory" => "releaseHistory",
                "installModule" => "installModule",
                "uninstallModule" => "uninstallModule",
                "upgradeModule" => "upgradeModule",
                "rollbackBuild" => "rollbackBuild",
                "toggleModule" => "toggleModule",
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
            }
            ModuleAuthority::Manage => {
                has_effective_permission(permissions, &Permission::MODULES_MANAGE)
            }
        }
    }

    fn permission_hint(self) -> &'static str {
        match self.authority {
            ModuleAuthority::Read => "modules:read or modules:list",
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
    use super::{ModuleAuthority, ModuleGraphqlDocumentPolicy, classify_document};
    use async_graphql::Request;

    #[test]
    fn classifies_module_fields_inside_fragments_and_subscriptions() {
        let mut request = Request::new(
            r#"
                query ModuleState { ...ModuleFields }
                fragment ModuleFields on Query { moduleRegistry { moduleSlug } }
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
            |field| field.name == "moduleRegistry" && field.authority == ModuleAuthority::Read
        ));
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
}
