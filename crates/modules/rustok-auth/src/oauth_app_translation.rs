use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Translation lifecycle for OAuth application presentation copy.
///
/// Revocation archives presentation while preserving the exact copy revision
/// that was visible before security lifecycle changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OAuthAppTranslationLifecycle {
    Active,
    Archived,
}

impl OAuthAppTranslationLifecycle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

pub fn oauth_app_translation_lifecycle(
    is_active: bool,
    revoked: bool,
) -> OAuthAppTranslationLifecycle {
    if is_active && !revoked {
        OAuthAppTranslationLifecycle::Active
    } else {
        OAuthAppTranslationLifecycle::Archived
    }
}

/// Semantic revision of all owner-localized OAuth application presentation copy.
///
/// Security/configuration fields intentionally do not participate. A secret
/// rotation, redirect/scopes/grants change, consent update or usage timestamp
/// therefore cannot invalidate a Translation proposal.
pub fn oauth_app_translation_resource_revision<I, S1, S2>(
    tenant_id: Uuid,
    app_id: Uuid,
    translations: I,
) -> String
where
    I: IntoIterator<Item = (S1, S2, Option<String>)>,
    S1: AsRef<str>,
    S2: AsRef<str>,
{
    let mut translations = translations
        .into_iter()
        .map(|(locale, name, description)| {
            (
                locale.as_ref().to_string(),
                name.as_ref().to_string(),
                description,
            )
        })
        .collect::<Vec<_>>();
    translations.sort_by(|left, right| left.0.cmp(&right.0));

    let mut digest = Sha256::new();
    append_text(&mut digest, "rustok-auth/oauth-app-translation-resource/v1");
    append_text(&mut digest, &tenant_id.to_string());
    append_text(&mut digest, &app_id.to_string());
    for (locale, name, description) in translations {
        append_copy(&mut digest, &locale, &name, description.as_deref());
    }
    format!("sha256:{}", hex::encode(digest.finalize()))
}

pub fn oauth_app_translation_locale_revision(
    tenant_id: Uuid,
    app_id: Uuid,
    locale: &str,
    name: &str,
    description: Option<&str>,
) -> String {
    let mut digest = Sha256::new();
    append_text(&mut digest, "rustok-auth/oauth-app-translation-locale/v1");
    append_text(&mut digest, &tenant_id.to_string());
    append_text(&mut digest, &app_id.to_string());
    append_copy(&mut digest, locale, name, description);
    format!("sha256:{}", hex::encode(digest.finalize()))
}

fn append_copy(digest: &mut Sha256, locale: &str, name: &str, description: Option<&str>) {
    append_text(digest, locale);
    append_text(digest, name);
    match description {
        Some(description) => {
            digest.update([1]);
            append_text(digest, description);
        }
        None => digest.update([0]),
    }
}

fn append_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_revision_is_locale_order_independent() {
        let tenant_id = Uuid::new_v4();
        let app_id = Uuid::new_v4();
        let first = oauth_app_translation_resource_revision(
            tenant_id,
            app_id,
            vec![
                ("fr", "Nom", Some("Description".to_string())),
                ("en", "Name", None),
            ],
        );
        let second = oauth_app_translation_resource_revision(
            tenant_id,
            app_id,
            vec![
                ("en", "Name", None),
                ("fr", "Nom", Some("Description".to_string())),
            ],
        );
        assert_eq!(first, second);
    }

    #[test]
    fn security_lifecycle_maps_to_translation_lifecycle() {
        assert_eq!(
            oauth_app_translation_lifecycle(true, false),
            OAuthAppTranslationLifecycle::Active
        );
        assert_eq!(
            oauth_app_translation_lifecycle(false, true),
            OAuthAppTranslationLifecycle::Archived
        );
    }
}
