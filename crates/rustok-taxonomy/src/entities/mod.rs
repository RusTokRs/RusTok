pub mod taxonomy_term;
pub mod taxonomy_term_alias;
pub mod taxonomy_term_route_key;
pub mod taxonomy_term_translation;
pub mod translation_change;

pub use taxonomy_term::Entity as TaxonomyTerm;
pub use taxonomy_term_alias::Entity as TaxonomyTermAlias;
pub use taxonomy_term_route_key::Entity as TaxonomyTermRouteKey;
pub use taxonomy_term_translation::Entity as TaxonomyTermTranslation;
pub use translation_change::Entity as TaxonomyTranslationChange;
