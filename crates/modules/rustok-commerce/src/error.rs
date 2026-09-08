pub use rustok_commerce_foundation::error::*;

impl From<rustok_core::Error>
    for crate::services::collection_translation::CollectionTranslationExactLocaleError
{
    fn from(error: rustok_core::Error) -> Self {
        Self::Commerce(CommerceError::Core(error))
    }
}
