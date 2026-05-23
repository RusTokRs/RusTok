use crate::model::PageDetail;

pub fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn parse_channel_slugs(value: &str) -> Vec<String> {
    let mut items = value
        .split(',')
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    items.sort();
    items.dedup();
    items
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{}: {}", context, error)
}

pub fn status_badge_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" => {
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        }
        "archived" => "bg-muted text-muted-foreground",
        _ => "bg-primary/10 text-primary",
    }
}

pub fn busy_key_with_id(action: &str, page_id: &str) -> String {
    format!("{}:{}", action, page_id)
}

pub fn busy_key_for_save(page_id: Option<&str>) -> String {
    page_id
        .map(|id| busy_key_with_id("save", id))
        .unwrap_or_else(|| "create".to_string())
}

#[derive(Debug, Clone)]
pub struct EditFormSeed {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub body: String,
    pub channel_slugs_text: String,
    pub publish_now: bool,
}

pub fn edit_form_seed_from_page(page: &PageDetail, default_locale: &str) -> EditFormSeed {
    let locale = page
        .translation
        .as_ref()
        .map(|translation| translation.locale.clone())
        .or_else(|| page.body.as_ref().map(|page_body| page_body.locale.clone()))
        .unwrap_or_else(|| default_locale.to_string());
    let title = page
        .translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .unwrap_or_default();
    let slug = page
        .translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .unwrap_or_default();
    let body = page
        .body
        .as_ref()
        .map(|page_body| page_body.content.clone())
        .unwrap_or_default();

    EditFormSeed {
        locale,
        title,
        slug,
        body,
        channel_slugs_text: page.channel_slugs.join(", "),
        publish_now: page.status.eq_ignore_ascii_case("published"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_ascii_words() {
        assert_eq!(slugify("Hello, Rustok Pages!"), "hello-rustok-pages");
    }

    #[test]
    fn parse_channel_slugs_trims_sorts_and_deduplicates() {
        assert_eq!(
            parse_channel_slugs(" web, mobile-app,WEB, , mobile-app "),
            vec!["mobile-app".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn helper_formatting_stays_consistent() {
        assert_eq!(
            error_with_context("Failed to save page", "timeout"),
            "Failed to save page: timeout"
        );
        assert_eq!(
            status_badge_class("published"),
            "bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
        assert_eq!(status_badge_class("draft"), "bg-primary/10 text-primary");
        assert_eq!(busy_key_with_id("edit", "p_1"), "edit:p_1");
        assert_eq!(busy_key_for_save(Some("p_2")), "save:p_2");
        assert_eq!(busy_key_for_save(None), "create");
        assert!(busy_key_matches_action(Some("save:p_1"), "save"));
        assert!(!busy_key_matches_action(Some("edit:p_1"), "save"));
        assert_eq!(
            status_badge_css("published"),
            "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400"
        );
    }

    #[test]
    fn label_with_id_replaces_placeholder() {
        assert_eq!(label_with_id("Editing page {id}", "page_1"), "Editing page page_1");
    }


    #[test]
    fn count_label_replaces_placeholder() {
        assert_eq!(count_label("{count} page(s)", 7), "7 page(s)");
    }


    #[test]
    fn empty_edit_form_seed_uses_default_locale() {
        let seed = empty_edit_form_seed("en");
        assert_eq!(seed.locale, "en");
        assert!(seed.title.is_empty());
        assert!(seed.slug.is_empty());
        assert!(seed.body.is_empty());
        assert!(seed.channel_slugs_text.is_empty());
        assert!(!seed.publish_now);
    }


    #[test]
    fn edit_form_seed_extracts_values() {
        let page = PageDetail {
            id: "p_1".to_string(),
            status: "published".to_string(),
            template: "default".to_string(),
            channel_slugs: vec!["web".to_string(), "mobile".to_string()],
            translation: Some(crate::model::PageTranslation {
                locale: "ru".to_string(),
                title: Some("Заголовок".to_string()),
                slug: Some("slug".to_string()),
            }),
            body: Some(crate::model::PageBody {
                locale: "ru".to_string(),
                content: "Body".to_string(),
                format: "markdown".to_string(),
            }),
        };

        let seed = edit_form_seed_from_page(&page, "en");
        assert_eq!(seed.locale, "ru");
        assert_eq!(seed.title, "Заголовок");
        assert_eq!(seed.slug, "slug");
        assert_eq!(seed.body, "Body");
        assert_eq!(seed.channel_slugs_text, "web, mobile");
        assert!(seed.publish_now);
    }
}

pub fn status_badge_css(status: &str) -> String {
    format!(
        "inline-flex rounded-full px-2.5 py-0.5 text-xs font-semibold {}",
        status_badge_class(status)
    )
}

pub fn busy_key_matches_action(busy_key: Option<&str>, action: &str) -> bool {
    let prefix = format!("{}:", action);
    busy_key
        .map(|key| key.starts_with(prefix.as_str()))
        .unwrap_or(false)
}


pub fn empty_edit_form_seed(default_locale: &str) -> EditFormSeed {
    EditFormSeed {
        locale: default_locale.to_string(),
        title: String::new(),
        slug: String::new(),
        body: String::new(),
        channel_slugs_text: String::new(),
        publish_now: false,
    }
}


pub fn count_label(template: &str, count: u64) -> String {
    template.replace("{count}", &count.to_string())
}


pub fn label_with_id(template: &str, id: &str) -> String {
    template.replace("{id}", id)
}
