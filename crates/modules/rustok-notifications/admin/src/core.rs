use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationsAdminPhase {
    SourceRegistry,
    Persistence,
    Delivery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationsAdminItemStatus {
    Ready,
    Unavailable,
    Planned,
}

impl NotificationsAdminItemStatus {
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::Ready => "notifications.status.ready",
            Self::Unavailable => "notifications.status.unavailable",
            Self::Planned => "notifications.status.planned",
        }
    }

    pub const fn default_label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Unavailable => "Unavailable",
            Self::Planned => "Planned",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationsAdminStatus {
    pub phase: NotificationsAdminPhase,
    pub source_registry_ready: bool,
    pub persistence_ready: bool,
    pub delivery_ready: bool,
}

impl NotificationsAdminStatus {
    pub const fn foundation() -> Self {
        Self {
            phase: NotificationsAdminPhase::SourceRegistry,
            source_registry_ready: true,
            persistence_ready: false,
            delivery_ready: false,
        }
    }

    pub const fn source_status(&self) -> NotificationsAdminItemStatus {
        if self.source_registry_ready {
            NotificationsAdminItemStatus::Ready
        } else {
            NotificationsAdminItemStatus::Unavailable
        }
    }

    pub const fn persistence_status(&self) -> NotificationsAdminItemStatus {
        if self.persistence_ready {
            NotificationsAdminItemStatus::Ready
        } else {
            NotificationsAdminItemStatus::Planned
        }
    }

    pub const fn delivery_status(&self) -> NotificationsAdminItemStatus {
        if self.delivery_ready {
            NotificationsAdminItemStatus::Ready
        } else {
            NotificationsAdminItemStatus::Planned
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foundation_status_has_correct_defaults_and_item_statuses() {
        let status = NotificationsAdminStatus::foundation();
        assert_eq!(status.phase, NotificationsAdminPhase::SourceRegistry);
        assert!(status.source_registry_ready);
        assert!(!status.persistence_ready);
        assert!(!status.delivery_ready);

        assert_eq!(
            status.source_status(),
            NotificationsAdminItemStatus::Ready
        );
        assert_eq!(
            status.source_status().i18n_key(),
            "notifications.status.ready"
        );
        assert_eq!(status.source_status().default_label(), "Ready");

        assert_eq!(
            status.persistence_status(),
            NotificationsAdminItemStatus::Planned
        );
        assert_eq!(
            status.persistence_status().i18n_key(),
            "notifications.status.planned"
        );
        assert_eq!(status.persistence_status().default_label(), "Planned");

        assert_eq!(
            status.delivery_status(),
            NotificationsAdminItemStatus::Planned
        );
        assert_eq!(
            status.delivery_status().i18n_key(),
            "notifications.status.planned"
        );
        assert_eq!(status.delivery_status().default_label(), "Planned");
    }

    #[test]
    fn custom_status_resolves_all_item_branches() {
        let status = NotificationsAdminStatus {
            phase: NotificationsAdminPhase::Delivery,
            source_registry_ready: false,
            persistence_ready: true,
            delivery_ready: true,
        };
        assert_eq!(
            status.source_status(),
            NotificationsAdminItemStatus::Unavailable
        );
        assert_eq!(
            status.source_status().i18n_key(),
            "notifications.status.unavailable"
        );
        assert_eq!(status.source_status().default_label(), "Unavailable");

        assert_eq!(
            status.persistence_status(),
            NotificationsAdminItemStatus::Ready
        );
        assert_eq!(
            status.delivery_status(),
            NotificationsAdminItemStatus::Ready
        );
    }

    #[test]
    fn status_and_enums_serde_roundtrips() {
        let status = NotificationsAdminStatus::foundation();
        let serialized = serde_json::to_string(&status).expect("serialize");
        let deserialized: NotificationsAdminStatus =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(status, deserialized);

        for item in [
            NotificationsAdminItemStatus::Ready,
            NotificationsAdminItemStatus::Unavailable,
            NotificationsAdminItemStatus::Planned,
        ] {
            let json = serde_json::to_string(&item).expect("serialize item");
            let back: NotificationsAdminItemStatus =
                serde_json::from_str(&json).expect("deserialize item");
            assert_eq!(item, back);
        }
    }
}
