use crate::forum_storefront_execution::ForumStorefrontSearchExecutionError;

const FORUM_STOREFRONT_SEARCH_UNAVAILABLE: &str =
    "Forum storefront Search is temporarily unavailable";

impl ForumStorefrontSearchExecutionError {
    /// Returns the bounded message that public transports may expose. Internal
    /// Search, database, provider, and serialization details remain server-only.
    pub fn public_message(&self) -> String {
        match self {
            Self::Validation(message) => message.clone(),
            Self::Scope(error) => match error.kind {
                rustok_api::PortErrorKind::Validation
                | rustok_api::PortErrorKind::NotFound
                | rustok_api::PortErrorKind::Forbidden => error.message.clone(),
                _ => FORUM_STOREFRONT_SEARCH_UNAVAILABLE.to_string(),
            },
            Self::Search(
                rustok_core::Error::Validation(message)
                | rustok_core::Error::NotFound(message)
                | rustok_core::Error::InvalidIdFormat(message),
            ) => message.clone(),
            Self::Search(_) | Self::Database(_) => {
                FORUM_STOREFRONT_SEARCH_UNAVAILABLE.to_string()
            }
        }
    }
}
