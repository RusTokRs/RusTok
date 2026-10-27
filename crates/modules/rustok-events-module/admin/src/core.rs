use crate::i18n;
use crate::model::EventDeliveryConfiguration;

pub const PROFILE_OUTBOX: &str = "outbox";
pub const PROFILE_OUTBOX_IGGY: &str = "outbox_iggy";

pub const KNOWN_PROFILES: [&str; 2] = [PROFILE_OUTBOX, PROFILE_OUTBOX_IGGY];

pub fn is_known_profile(value: &str) -> bool {
    KNOWN_PROFILES.contains(&value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSelectionDecision {
    Select(String),
    RequiresIggyDialog,
}

pub fn evaluate_profile_selection(
    desired_profile: &str,
    iggy_configured: bool,
) -> ProfileSelectionDecision {
    if desired_profile == PROFILE_OUTBOX_IGGY && !iggy_configured {
        ProfileSelectionDecision::RequiresIggyDialog
    } else {
        ProfileSelectionDecision::Select(desired_profile.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDeliverySummaryViewModel {
    pub active_profile: String,
    pub desired_profile: String,
    pub connector_status: String,
}

impl EventDeliverySummaryViewModel {
    pub fn from_configuration(
        config: &EventDeliveryConfiguration,
        labels: &EventsAdminLabels,
    ) -> Self {
        Self {
            active_profile: config.active_profile.clone(),
            desired_profile: config.desired_profile.clone(),
            connector_status: labels
                .format_connector_status(&config.iggy_mode, config.iggy_configured),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventsAdminLabels {
    pub title: String,
    pub subtitle: String,
    pub header_eyebrow: String,
    pub profile_outbox: String,
    pub profile_outbox_description: String,
    pub profile_outbox_iggy: String,
    pub profile_outbox_iggy_description: String,
    pub active_profile: String,
    pub desired_profile: String,
    pub iggy_connector: String,
    pub iggy_ready: String,
    pub iggy_config_required: String,
    pub save_profile: String,
    pub saving: String,
    pub saved_restart_required: String,
    pub saved_already_active: String,
    pub iggy_dialog_title: String,
    pub iggy_dialog_body: String,
    pub iggy_dialog_open: String,
    pub iggy_dialog_cancel: String,
}

impl EventsAdminLabels {
    pub fn resolve(locale: Option<&str>) -> Self {
        Self {
            title: i18n::t(locale, "events-title", "Events"),
            subtitle: i18n::t(
                locale,
                "events-subtitle",
                "Choose one global delivery profile. Iggy connection settings remain owned by the Iggy Connector module.",
            ),
            header_eyebrow: i18n::t(locale, "events-header-eyebrow", "Platform module"),
            profile_outbox: i18n::t(locale, "events-profile-outbox", "Outbox"),
            profile_outbox_description: i18n::t(
                locale,
                "events-profile-outbox-description",
                "Transactional database outbox with a lightweight in-process relay.",
            ),
            profile_outbox_iggy: i18n::t(locale, "events-profile-outbox-iggy", "Outbox + Iggy"),
            profile_outbox_iggy_description: i18n::t(
                locale,
                "events-profile-outbox-iggy-description",
                "Transactional outbox with Iggy relay for high-throughput workloads.",
            ),
            active_profile: i18n::t(locale, "events-active-profile", "Active profile"),
            desired_profile: i18n::t(locale, "events-desired-profile", "Desired profile"),
            iggy_connector: i18n::t(locale, "events-iggy-connector", "Iggy connector"),
            iggy_ready: i18n::t(locale, "events-iggy-ready", "ready"),
            iggy_config_required: i18n::t(
                locale,
                "events-iggy-config-required",
                "configuration required",
            ),
            save_profile: i18n::t(locale, "events-save-profile", "Save profile"),
            saving: i18n::t(locale, "events-saving", "Saving..."),
            saved_restart_required: i18n::t(
                locale,
                "events-saved-restart-required",
                "Saved. Restart the server to activate this profile.",
            ),
            saved_already_active: i18n::t(
                locale,
                "events-saved-already-active",
                "Saved. This profile is already active.",
            ),
            iggy_dialog_title: i18n::t(locale, "events-iggy-dialog-title", "Configure Iggy first"),
            iggy_dialog_body: i18n::t(
                locale,
                "events-iggy-dialog-body",
                "The outbox_iggy profile requires a ready bundled or external Iggy connection.",
            ),
            iggy_dialog_open: i18n::t(locale, "events-iggy-dialog-open", "Open Iggy Connector"),
            iggy_dialog_cancel: i18n::t(locale, "events-iggy-dialog-cancel", "Cancel"),
        }
    }

    pub fn profile_label<'a>(&'a self, profile_id: &'a str) -> &'a str {
        match profile_id {
            PROFILE_OUTBOX => &self.profile_outbox,
            PROFILE_OUTBOX_IGGY => &self.profile_outbox_iggy,
            _ => profile_id,
        }
    }

    pub fn profile_description(&self, profile_id: &str) -> &str {
        match profile_id {
            PROFILE_OUTBOX => &self.profile_outbox_description,
            PROFILE_OUTBOX_IGGY => &self.profile_outbox_iggy_description,
            _ => "",
        }
    }

    pub fn format_connector_status(&self, mode: &str, is_configured: bool) -> String {
        let state = if is_configured {
            &self.iggy_ready
        } else {
            &self.iggy_config_required
        };
        format!("{mode} ({state})")
    }

    pub fn format_save_outcome(&self, restart_required: bool) -> &str {
        if restart_required {
            &self.saved_restart_required
        } else {
            &self.saved_already_active
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_and_unknown_profiles() {
        assert!(is_known_profile(PROFILE_OUTBOX));
        assert!(is_known_profile(PROFILE_OUTBOX_IGGY));
        assert!(!is_known_profile("direct"));
        assert!(!is_known_profile("kafka"));
        assert!(!is_known_profile(""));
    }

    #[test]
    fn test_evaluate_profile_selection() {
        assert_eq!(
            evaluate_profile_selection(PROFILE_OUTBOX, false),
            ProfileSelectionDecision::Select(PROFILE_OUTBOX.to_string())
        );
        assert_eq!(
            evaluate_profile_selection(PROFILE_OUTBOX, true),
            ProfileSelectionDecision::Select(PROFILE_OUTBOX.to_string())
        );
        assert_eq!(
            evaluate_profile_selection(PROFILE_OUTBOX_IGGY, false),
            ProfileSelectionDecision::RequiresIggyDialog
        );
        assert_eq!(
            evaluate_profile_selection(PROFILE_OUTBOX_IGGY, true),
            ProfileSelectionDecision::Select(PROFILE_OUTBOX_IGGY.to_string())
        );
    }

    #[test]
    fn test_summary_view_model_from_configuration() {
        let config = EventDeliveryConfiguration {
            active_profile: "outbox".to_string(),
            desired_profile: "outbox_iggy".to_string(),
            iggy_mode: "external".to_string(),
            iggy_configured: true,
            restart_required: true,
        };
        let labels = EventsAdminLabels::resolve(Some("en"));
        let vm = EventDeliverySummaryViewModel::from_configuration(&config, &labels);
        assert_eq!(vm.active_profile, "outbox");
        assert_eq!(vm.desired_profile, "outbox_iggy");
        assert_eq!(vm.connector_status, "external (ready)");
    }

    #[test]
    fn test_labels_resolution() {
        let labels_en = EventsAdminLabels::resolve(Some("en"));
        assert_eq!(labels_en.title, "Events");
        assert_eq!(labels_en.save_profile, "Save profile");
        assert_eq!(labels_en.profile_label(PROFILE_OUTBOX), "Outbox");
        assert_eq!(
            labels_en.profile_description(PROFILE_OUTBOX),
            "Transactional database outbox with a lightweight in-process relay."
        );
        assert_eq!(
            labels_en.format_connector_status("bundled", true),
            "bundled (ready)"
        );
        assert_eq!(
            labels_en.format_save_outcome(true),
            "Saved. Restart the server to activate this profile."
        );
        assert_eq!(
            labels_en.format_save_outcome(false),
            "Saved. This profile is already active."
        );

        let labels_ru = EventsAdminLabels::resolve(Some("ru"));
        assert_eq!(labels_ru.title, "События");
        assert_eq!(labels_ru.save_profile, "Сохранить профиль");
        assert_eq!(labels_ru.profile_label(PROFILE_OUTBOX), "Outbox");
        assert_eq!(
            labels_ru.format_connector_status("bundled", true),
            "bundled (готов)"
        );
        assert_eq!(
            labels_ru.format_save_outcome(true),
            "Сохранено. Перезапустите сервер для активации профиля."
        );
        assert_eq!(
            labels_ru.format_save_outcome(false),
            "Сохранено. Этот профиль уже активен."
        );

        let labels_default = EventsAdminLabels::resolve(None);
        assert_eq!(labels_default.title, "Events");
    }
}
