pub const AI_GRAPHQL_CONTRIBUTION: rustok_api::graphql::GraphqlContributionDescriptor =
    rustok_api::graphql::GraphqlContributionDescriptor::new(
        Some("graphql::AiQuery"),
        Some("graphql::AiMutation"),
        Some("graphql::AiSubscription"),
        Some("graphql_runtime::attach_schema_data"),
    );

/// Single typed GraphQL context value owned by the AI capability.
#[cfg(feature = "server")]
#[derive(Clone)]
pub struct AiGraphqlRuntimeData {
    runtime: crate::AiHostRuntime,
}

#[cfg(feature = "server")]
impl AiGraphqlRuntimeData {
    pub fn runtime(&self) -> &crate::AiHostRuntime {
        &self.runtime
    }
}

/// Capability-owned factory consumed by manifest-generated schema composition.
#[cfg(feature = "server")]
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<AiGraphqlRuntimeData, String> {
    let runtime = crate::ai_host_runtime_from_context(inputs.host())?;
    Ok(AiGraphqlRuntimeData { runtime })
}
