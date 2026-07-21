use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(format!("unsupported {} value: {value}", stringify!($name))),
                }
            }
        }
    };
}

string_enum!(GroupVisibility {
    Public => "public",
    Closed => "closed",
    Secret => "secret",
});

string_enum!(GroupJoinPolicy {
    Open => "open",
    Request => "request",
    InviteOnly => "invite_only",
});

string_enum!(GroupStatus {
    Active => "active",
    Archived => "archived",
    Suspended => "suspended",
});

string_enum!(GroupRole {
    Owner => "owner",
    Admin => "admin",
    Moderator => "moderator",
    Member => "member",
});

string_enum!(GroupMembershipStatus {
    Active => "active",
    Pending => "pending",
    Invited => "invited",
    Banned => "banned",
    Left => "left",
});

string_enum!(GroupFeatureStatus {
    Enabled => "enabled",
    Disabled => "disabled",
});

string_enum!(GroupAction {
    Discover => "discover",
    View => "view",
    ViewMembers => "view_members",
    Join => "join",
    Post => "post",
    Comment => "comment",
    Invite => "invite",
    ReviewMemberships => "review_memberships",
    Moderate => "moderate",
    ManageFeatures => "manage_features",
    ManageSettings => "manage_settings",
    TransferOwnership => "transfer_ownership",
});

impl GroupRole {
    pub const fn can_manage_settings(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub const fn can_moderate(self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Moderator)
    }
}

pub fn normalize_group_handle(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase().replace(' ', "-");
    if !(3..=80).contains(&normalized.len()) {
        return Err("group handle must contain between 3 and 80 characters".to_string());
    }
    if normalized.starts_with('-')
        || normalized.ends_with('-')
        || !normalized
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-' || character == '_')
    {
        return Err("group handle may contain lowercase ASCII letters, digits, hyphens, and underscores".to_string());
    }
    Ok(normalized)
}

pub fn normalize_feature_key(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    let Some((owner, feature)) = normalized.split_once('.') else {
        return Err("group feature key must be namespaced, for example forum.discussions".to_string());
    };
    let valid_part = |part: &str| {
        !part.is_empty()
            && part.len() <= 64
            && part
                .chars()
                .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' || character == '-')
    };
    if !valid_part(owner) || !valid_part(feature) {
        return Err("group feature namespace and name contain unsupported characters".to_string());
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_normalized_without_locale_rules() {
        assert_eq!(normalize_group_handle(" Rust-Users ").unwrap(), "rust-users");
        assert!(normalize_group_handle("секция").is_err());
        assert!(normalize_group_handle("a").is_err());
    }

    #[test]
    fn feature_keys_require_an_owner_namespace() {
        assert_eq!(normalize_feature_key("Forum.Discussions").unwrap(), "forum.discussions");
        assert!(normalize_feature_key("forum").is_err());
    }
}
