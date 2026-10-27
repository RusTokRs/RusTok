//! Core domain and presentation logic for the Page Builder storefront package.
//! Framework-agnostic: zero UI framework dependencies.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderStorefrontViewModel {
    pub title: String,
    pub description: String,
    pub is_empty: bool,
}

pub fn page_builder_view_model(
    locale: Option<&str>,
    custom_title: Option<&str>,
    has_content: bool,
) -> PageBuilderStorefrontViewModel {
    let title = custom_title
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.to_string())
        .unwrap_or_else(|| crate::i18n::t(locale, "page_builder-title", "Page Builder"));
    let description = if has_content {
        String::new()
    } else {
        crate::i18n::t(
            locale,
            "page_builder-render_error",
            "Unable to render visual page",
        )
    };
    PageBuilderStorefrontViewModel {
        title,
        description,
        is_empty: !has_content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_model_defaults_to_localized_strings_when_no_content() {
        let vm = page_builder_view_model(Some("en"), None, false);
        assert_eq!(vm.title, "Page Builder");
        assert_eq!(vm.description, "Unable to render visual page");
        assert!(vm.is_empty);

        let vm_ru = page_builder_view_model(Some("ru"), None, false);
        assert_eq!(vm_ru.title, "Конструктор страниц");
        assert_eq!(
            vm_ru.description,
            "Не удалось отобразить визуальную страницу"
        );
        assert!(vm_ru.is_empty);
    }

    #[test]
    fn view_model_preserves_custom_title_and_clears_error_when_has_content() {
        let vm = page_builder_view_model(Some("en"), Some("Custom Landing"), true);
        assert_eq!(vm.title, "Custom Landing");
        assert_eq!(vm.description, "");
        assert!(!vm.is_empty);
    }

    #[test]
    fn view_model_trims_whitespace_custom_title() {
        let vm = page_builder_view_model(Some("en"), Some("   "), false);
        assert_eq!(vm.title, "Page Builder");
        assert_eq!(vm.description, "Unable to render visual page");
        assert!(vm.is_empty);
    }
}
