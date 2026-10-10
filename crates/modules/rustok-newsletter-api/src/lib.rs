//! Newsletter API contracts crate.
//!
//! Defines the neutral content provider and subscriber port contracts that
//! source modules (blog, forum, commerce) implement to supply content for
//! newsletter campaigns.

mod model;
#[cfg(feature = "server")]
mod provider;
#[cfg(feature = "server")]
mod port;

pub use model::*;
#[cfg(feature = "server")]
pub use provider::*;
#[cfg(feature = "server")]
pub use port::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_source_slug_accepts_valid_values() {
        assert!(ContentSourceSlug::new("blog").is_ok());
        assert!(ContentSourceSlug::new("forum").is_ok());
        assert!(ContentSourceSlug::new("commerce").is_ok());
    }

    #[test]
    fn content_source_slug_rejects_invalid_values() {
        assert!(ContentSourceSlug::new("").is_err());
        assert!(ContentSourceSlug::new("Blog").is_err());
        assert!(ContentSourceSlug::new(" blog").is_err());
        assert!(ContentSourceSlug::new("blog/extra").is_err());
    }
}
