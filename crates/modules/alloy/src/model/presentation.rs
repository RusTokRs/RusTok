use chrono::{DateTime, FixedOffset};
use rustok_api::StoredLocale;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Owner-localized presentation copy for one Alloy Script and one locale.
///
/// Runtime identity, source/workspace, triggers, permissions and execution
/// evidence are intentionally absent from this model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptPresentation {
    pub tenant_id: Uuid,
    pub script_id: Uuid,
    pub locale: StoredLocale,
    pub description: Option<String>,
    pub copy_revision: i64,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

/// Semantic revision for one localized presentation row.
///
/// The stored monotonic `copy_revision` is deliberately not included in the
/// digest: exact replay of identical copy has the same semantic revision.
pub fn script_presentation_locale_revision(
    tenant_id: Uuid,
    script_id: Uuid,
    locale: &StoredLocale,
    description: Option<&str>,
) -> String {
    let mut digest = Sha256::new();
    append_text(&mut digest, "alloy/script-presentation-locale/v1");
    append_text(&mut digest, &tenant_id.to_string());
    append_text(&mut digest, &script_id.to_string());
    append_text(&mut digest, locale.as_str());
    append_optional_text(&mut digest, description);
    format!("sha256:{}", hex::encode(digest.finalize()))
}

/// Semantic revision of all owner-localized presentation copy for a Script.
pub fn script_presentation_resource_revision<I>(
    tenant_id: Uuid,
    script_id: Uuid,
    presentations: I,
) -> String
where
    I: IntoIterator<Item = (StoredLocale, Option<String>)>,
{
    let mut presentations = presentations.into_iter().collect::<Vec<_>>();
    presentations.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));

    let mut digest = Sha256::new();
    append_text(&mut digest, "alloy/script-presentation-resource/v1");
    append_text(&mut digest, &tenant_id.to_string());
    append_text(&mut digest, &script_id.to_string());
    for (locale, description) in presentations {
        append_text(&mut digest, locale.as_str());
        append_optional_text(&mut digest, description.as_deref());
    }
    format!("sha256:{}", hex::encode(digest.finalize()))
}

fn append_optional_text(digest: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            digest.update([1]);
            append_text(digest, value);
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
        let script_id = Uuid::new_v4();
        let en = StoredLocale::new("en").unwrap();
        let fr = StoredLocale::new("fr").unwrap();

        let first = script_presentation_resource_revision(
            tenant_id,
            script_id,
            vec![
                (fr.clone(), Some("Description".to_string())),
                (en.clone(), Some("Description".to_string())),
            ],
        );
        let second = script_presentation_resource_revision(
            tenant_id,
            script_id,
            vec![
                (en, Some("Description".to_string())),
                (fr, Some("Description".to_string())),
            ],
        );

        assert_eq!(first, second);
    }

    #[test]
    fn unknown_provenance_locale_is_truthful_and_hashable() {
        let tenant_id = Uuid::new_v4();
        let script_id = Uuid::new_v4();
        let locale = StoredLocale::new("und").unwrap();

        assert!(locale.is_unknown_provenance());
        assert_eq!(
            script_presentation_locale_revision(
                tenant_id,
                script_id,
                &locale,
                Some("legacy description")
            ),
            script_presentation_locale_revision(
                tenant_id,
                script_id,
                &locale,
                Some("legacy description")
            )
        );
    }
}
