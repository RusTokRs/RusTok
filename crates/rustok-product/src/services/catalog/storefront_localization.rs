use crate::dto::{ProductImageTranslationResponse, ProductResponse};
use rustok_api::locale_tags_match;

use super::helpers;

pub(super) fn localize_product_response(
    product: ProductResponse,
    locale: &str,
    fallback_locale: &str,
) -> ProductResponse {
    let mut product = helpers::localize_product_response(product, locale, fallback_locale);

    for image in &mut product.images {
        let localized_alt_text = pick_image_translation(
            image.translations.as_slice(),
            locale,
            fallback_locale,
        )
        .map(|translation| translation.alt_text.clone());

        if let Some(alt_text) = localized_alt_text {
            // A localized row with `None` is an explicit empty alt text. The base column is
            // only a compatibility fallback when no localized row can be selected at all.
            image.alt_text = alt_text;
        }
    }

    product
}

fn pick_image_translation<'a>(
    translations: &'a [ProductImageTranslationResponse],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a ProductImageTranslationResponse> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(fallback_locale, locale)).then(|| {
                translations.iter().find(|translation| {
                    locale_tags_match(&translation.locale, fallback_locale)
                })
            })?
        })
        .or_else(|| translations.first())
}

#[cfg(test)]
mod tests {
    use super::pick_image_translation;
    use crate::dto::ProductImageTranslationResponse;

    fn translation(locale: &str, alt_text: Option<&str>) -> ProductImageTranslationResponse {
        ProductImageTranslationResponse {
            locale: locale.to_string(),
            alt_text: alt_text.map(str::to_string),
        }
    }

    #[test]
    fn image_translation_prefers_requested_then_fallback_locale() {
        let translations = vec![
            translation("en-US", Some("English alt")),
            translation("fr-FR", Some("Texte alternatif")),
        ];

        let requested = pick_image_translation(&translations, "fr-FR", "en-US")
            .expect("requested image translation");
        assert_eq!(requested.locale, "fr-FR");

        let fallback = pick_image_translation(&translations, "de-DE", "en-US")
            .expect("fallback image translation");
        assert_eq!(fallback.locale, "en-US");
    }

    #[test]
    fn image_translation_preserves_explicit_empty_alt_text() {
        let translations = vec![
            translation("en-US", Some("English alt")),
            translation("fr-FR", None),
        ];

        let selected = pick_image_translation(&translations, "fr-FR", "en-US")
            .expect("requested image translation");
        assert_eq!(selected.alt_text, None);
    }
}
