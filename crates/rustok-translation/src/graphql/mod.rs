mod context;
mod mutation;
mod query;
mod types;

pub use mutation::TranslationMutation;
pub use query::TranslationQuery;

pub const TRANSLATION_GRAPHQL_CONTRIBUTION: rustok_api::graphql::GraphqlContributionDescriptor =
    rustok_api::graphql::GraphqlContributionDescriptor::new(
        Some("graphql::TranslationQuery"),
        Some("graphql::TranslationMutation"),
        None,
        Some("graphql_runtime::attach_schema_data"),
    );

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_graphql::{EmptySubscription, Schema};
    use rustok_core::events::MemoryTransport;
    use rustok_outbox::TransactionalEventBus;
    use sea_orm::Database;

    use super::{TranslationMutation, TranslationQuery};

    #[tokio::test]
    async fn schema_publishes_translation_control_plane() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
        let host = rustok_api::HostRuntimeContext::new(database).with_shared_value(event_bus);
        let inputs = rustok_api::graphql::GraphqlRuntimeInputs::new(host);
        let runtime =
            crate::graphql_runtime::attach_schema_data(&inputs).expect("translation runtime");
        let schema = Schema::build(TranslationQuery, TranslationMutation, EmptySubscription)
            .data(runtime)
            .finish();
        let sdl = schema.sdl();

        for field in [
            "translationPolicy",
            "translationTargets",
            "translationGlossaries",
            "translationGlossary",
            "translationMemoryEntries",
            "translationMemoryEntry",
            "translationMemorySuggestions",
            "machineTranslationOperationStatus",
            "translationJobProgress",
            "translationReviewerQueue",
            "translationReviewerWorkload",
            "exportTranslationJob",
            "translationRequiredProviderProgress",
            "replaceTranslationPolicy",
            "createTranslationGlossary",
            "updateTranslationGlossary",
            "replaceTranslationGlossaryTerms",
            "setTranslationGlossaryActive",
            "setTranslationMemoryRetention",
            "tombstoneTranslationMemoryEntry",
            "purgeTranslationMemoryEntry",
            "createTranslationJob",
            "saveTranslationProposal",
            "importTranslationItem",
            "generateMachineTranslationProposal",
            "cancelMachineTranslationOperation",
            "recoverMachineTranslationOperation",
            "applyTranslationProposal",
            "syncTranslationProviderInventory",
        ] {
            assert!(sdl.contains(field), "missing GraphQL field {field}");
        }
    }
}
