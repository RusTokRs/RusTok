#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HeaderLinks {
    pub home_href: String,
    pub english_href: String,
    pub russian_href: String,
}

pub(super) fn build_header_links(locale: &str) -> HeaderLinks {
    HeaderLinks {
        home_href: storefront_root_for_locale(locale),
        english_href: storefront_root_for_locale("en"),
        russian_href: storefront_root_for_locale("ru"),
    }
}

fn storefront_root_for_locale(locale: &str) -> String {
    format!("/{}", locale.trim().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::build_header_links;

    #[test]
    fn header_links_normalize_home_and_language_targets() {
        let links = build_header_links(" RU ");

        assert_eq!(links.home_href, "/ru");
        assert_eq!(links.english_href, "/en");
        assert_eq!(links.russian_href, "/ru");
    }
}
