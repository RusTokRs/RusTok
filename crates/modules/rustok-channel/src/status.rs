//! Channel publication state with a single source of truth.
//!
//! A channel used to carry a free-form `status` column that every writer left at
//! the hard-coded literal `"experimental"`, while the column was also a required,
//! filterable field of the Sales Channel Index schema. That made the indexed value
//! permanently untrue and gave the same fact two writers (`is_active` and
//! `status`) that could disagree.
//!
//! The publication state is now *derived*: `is_active` is the only stored fact and
//! [`ChannelStatus`] is the typed projection of it for API responses and index
//! projections. Nothing can store a status that contradicts `is_active`.

use serde::{Deserialize, Serialize};

/// Publication state of a channel, derived from `channels.is_active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    /// The channel participates in resolution and storefront traffic.
    Active,
    /// The channel is retained for administration but never resolved.
    Inactive,
}

impl ChannelStatus {
    /// Every status the platform can expose, in a stable presentation order.
    pub const ALL: [Self; 2] = [Self::Active, Self::Inactive];

    /// Derives the publication state from the stored `is_active` flag.
    pub const fn from_is_active(is_active: bool) -> Self {
        if is_active {
            Self::Active
        } else {
            Self::Inactive
        }
    }

    /// Returns the stored `is_active` flag this status projects.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    /// Stable wire/representation value used by the index schema and API payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }

    /// Parses the stable wire value, rejecting anything outside the closed set.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "active" => Some(Self::Active),
            "inactive" => Some(Self::Inactive),
            _ => None,
        }
    }
}

impl std::fmt::Display for ChannelStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<bool> for ChannelStatus {
    fn from(is_active: bool) -> Self {
        Self::from_is_active(is_active)
    }
}

#[cfg(test)]
mod tests {
    use super::ChannelStatus;

    #[test]
    fn status_is_the_typed_projection_of_is_active() {
        for is_active in [true, false] {
            let status = ChannelStatus::from_is_active(is_active);
            assert_eq!(status.is_active(), is_active);
            assert_eq!(ChannelStatus::parse(status.as_str()), Some(status));
            assert_eq!(status.to_string(), status.as_str());
        }
        assert_eq!(ChannelStatus::ALL.len(), 2);
    }

    #[test]
    fn status_rejects_the_retired_free_form_values() {
        assert_eq!(ChannelStatus::parse("experimental"), None);
        assert_eq!(ChannelStatus::parse(""), None);
        assert_eq!(ChannelStatus::parse(" ACTIVE "), Some(ChannelStatus::Active));
    }
}
