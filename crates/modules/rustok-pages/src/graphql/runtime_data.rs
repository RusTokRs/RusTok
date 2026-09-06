use rustok_api::graphql::GraphqlRuntimeInputs;
use rustok_page_builder::health::ProviderHealthSnapshot;

use crate::SharedPagesProviderHealthAuthority;

/// Manifest-attached Pages GraphQL runtime data.
///
/// Provider health is absent unless the host published a deployment-bound, owner-accepted
/// authority through `ModuleRuntimeExtensions`. Even when an authority exists, every read performs
/// a freshness check and returns `None` after the retained evaluator freshness window expires.
#[derive(Clone, Default)]
pub struct PagesGraphqlRuntimeData {
    provider_health_authority: Option<SharedPagesProviderHealthAuthority>,
}

pub fn attach_schema_data(
    inputs: &GraphqlRuntimeInputs,
) -> Result<PagesGraphqlRuntimeData, String> {
    Ok(PagesGraphqlRuntimeData {
        provider_health_authority: inputs.shared_get::<SharedPagesProviderHealthAuthority>(),
    })
}

impl PagesGraphqlRuntimeData {
    pub(crate) fn provider_health_snapshot(&self) -> Option<ProviderHealthSnapshot> {
        self.provider_health_authority
            .as_ref()
            .and_then(|authority| authority.current_snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_data_factory_is_manifest_attachable() {
        let factory: fn(&GraphqlRuntimeInputs) -> Result<PagesGraphqlRuntimeData, String> =
            attach_schema_data;
        let selector: fn(&PagesGraphqlRuntimeData) -> Option<ProviderHealthSnapshot> =
            PagesGraphqlRuntimeData::provider_health_snapshot;
        let _ = (factory, selector);
    }
}
