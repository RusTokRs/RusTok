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

    #[test]
    fn subscriber_status_display_and_parse_roundtrip() {
        for status in [
            SubscriberStatus::Pending,
            SubscriberStatus::Active,
            SubscriberStatus::Unsubscribed,
            SubscriberStatus::Suppressed,
        ] {
            let str_val = status.as_str();
            assert_eq!(status.to_string(), str_val);
            let parsed: SubscriberStatus = str_val.parse().expect("valid subscriber status");
            assert_eq!(parsed, status);
        }
        assert!("invalid_status".parse::<SubscriberStatus>().is_err());
    }

    #[test]
    fn campaign_status_display_and_parse_roundtrip() {
        for status in [
            CampaignStatus::Draft,
            CampaignStatus::Scheduled,
            CampaignStatus::Sending,
            CampaignStatus::Sent,
            CampaignStatus::Cancelled,
        ] {
            let str_val = status.as_str();
            assert_eq!(status.to_string(), str_val);
            let parsed: CampaignStatus = str_val.parse().expect("valid campaign status");
            assert_eq!(parsed, status);
        }
        assert!("unknown".parse::<CampaignStatus>().is_err());
    }

    #[test]
    fn subscriber_and_campaign_status_serde_snake_case() {
        let json = serde_json::to_string(&SubscriberStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let deserialized: SubscriberStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SubscriberStatus::Active);

        let json = serde_json::to_string(&CampaignStatus::Draft).unwrap();
        assert_eq!(json, "\"draft\"");
        let deserialized: CampaignStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, CampaignStatus::Draft);
    }
}
