//! Domain state machines, value objects, and invariants for the newsletter module.

mod campaign_status;
mod subscriber_status;

pub use campaign_status::{CampaignLifecycle, CampaignTransition};
pub use subscriber_status::{SubscriberLifecycle, SubscriberTransition};
