#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShippingProfileFormState {
    pub editing_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub metadata_json: String,
}

pub fn shipping_profile_form_state(
    profile: &crate::model::ShippingProfile,
) -> ShippingProfileFormState {
    ShippingProfileFormState {
        editing_id: Some(profile.id.clone()),
        slug: profile.slug.clone(),
        name: profile.name.clone(),
        description: profile.description.clone().unwrap_or_default(),
        metadata_json: profile.metadata.clone(),
    }
}

pub fn empty_shipping_profile_form_state() -> ShippingProfileFormState {
    ShippingProfileFormState::default()
}

pub fn prepare_shipping_profile_draft(
    slug: &str,
    name: &str,
    description: &str,
    metadata_json: &str,
    locale: String,
) -> Option<crate::model::ShippingProfileDraft> {
    let slug = slug.trim().to_string();
    let name = name.trim().to_string();

    if slug.is_empty() || name.is_empty() {
        return None;
    }

    Some(crate::model::ShippingProfileDraft {
        slug,
        name,
        description: description.trim().to_string(),
        metadata_json: metadata_json.trim().to_string(),
        locale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipping_profile_form_state_maps_optional_description() {
        let profile = crate::model::ShippingProfile {
            id: "profile-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            slug: "standard".to_string(),
            name: "Standard".to_string(),
            description: None,
            active: true,
            metadata: r#"{"carrier":"ups"}"#.to_string(),
            created_at: "2026-06-07T00:00:00Z".to_string(),
            updated_at: "2026-06-07T00:00:00Z".to_string(),
        };

        let state = shipping_profile_form_state(&profile);

        assert_eq!(state.editing_id.as_deref(), Some("profile-1"));
        assert_eq!(state.slug, "standard");
        assert_eq!(state.name, "Standard");
        assert_eq!(state.description, "");
        assert_eq!(state.metadata_json, r#"{"carrier":"ups"}"#);
    }

    #[test]
    fn shipping_profile_draft_trims_and_requires_slug_and_name() {
        let draft = prepare_shipping_profile_draft(
            " standard ",
            " Standard ",
            " Delivery ",
            " { } ",
            "en".to_string(),
        )
        .expect("valid draft");

        assert_eq!(draft.slug, "standard");
        assert_eq!(draft.name, "Standard");
        assert_eq!(draft.description, "Delivery");
        assert_eq!(draft.metadata_json, "{ }");
        assert_eq!(draft.locale, "en");
        assert!(prepare_shipping_profile_draft("", "Name", "", "", "en".to_string()).is_none());
        assert!(prepare_shipping_profile_draft("slug", " ", "", "", "en".to_string()).is_none());
    }
}
