use async_graphql::Json;
use rustok_seo::{
    seo_builtin_slug, SeoBulkApplyInput, SeoBulkApplyMode, SeoBulkBoolFieldPatch,
    SeoBulkExportInput, SeoBulkFieldPatchMode, SeoBulkImportInput, SeoBulkJsonFieldPatch,
    SeoBulkListInput, SeoBulkMetaPatchInput, SeoBulkSelectionInput, SeoBulkSelectionMode,
    SeoBulkSource, SeoBulkStringFieldPatch, SeoModuleSettings, SeoRedirectInput,
    SeoRedirectMatchType, SeoTargetSlug, SeoTemplateRuleSet,
};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

pub const ROBOT_DIRECTIVE_PRESETS: &[&str] = &[
    "index",
    "follow",
    "noindex",
    "nofollow",
    "noarchive",
    "nosnippet",
    "noimageindex",
    "notranslate",
    "max-image-preview:large",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeoAdminTab {
    Bulk,
    Redirects,
    Sitemaps,
    Robots,
    Defaults,
    Diagnostics,
}

impl SeoAdminTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bulk => "bulk",
            Self::Redirects => "redirects",
            Self::Sitemaps => "sitemaps",
            Self::Robots => "robots",
            Self::Defaults => "defaults",
            Self::Diagnostics => "diagnostics",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "bulk" => Some(Self::Bulk),
            "redirects" => Some(Self::Redirects),
            "sitemaps" => Some(Self::Sitemaps),
            "robots" => Some(Self::Robots),
            "defaults" => Some(Self::Defaults),
            "diagnostics" => Some(Self::Diagnostics),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SeoBulkFilterForm {
    pub target_kind: SeoTargetSlug,
    pub locale: String,
    pub query: String,
    pub source: SeoBulkSource,
    pub page: i32,
    pub per_page: i32,
}

impl SeoBulkFilterForm {
    pub fn new(default_locale: Option<&str>) -> Self {
        Self {
            target_kind: SeoTargetSlug::new(seo_builtin_slug::PAGE)
                .expect("builtin SEO target slug must stay valid"),
            locale: default_locale.unwrap_or("en").to_string(),
            query: String::new(),
            source: SeoBulkSource::Any,
            page: 1,
            per_page: 20,
        }
    }

    pub fn build_input(&self) -> Result<SeoBulkListInput, String> {
        let locale = self.locale.trim();
        if locale.is_empty() {
            return Err("Bulk locale is required".to_string());
        }

        Ok(SeoBulkListInput {
            target_kind: self.target_kind.clone(),
            locale: locale.to_string(),
            query: trim_to_option(self.query.as_str()),
            source: Some(self.source),
            page: self.page.max(1),
            per_page: self.per_page.clamp(1, 100),
        })
    }
}

#[derive(Clone, Debug)]
pub struct SeoBulkStringPatchForm {
    pub mode: SeoBulkFieldPatchMode,
    pub value: String,
}

impl Default for SeoBulkStringPatchForm {
    fn default() -> Self {
        Self {
            mode: SeoBulkFieldPatchMode::Keep,
            value: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SeoBulkBoolPatchForm {
    pub mode: SeoBulkFieldPatchMode,
    pub value: bool,
}

impl Default for SeoBulkBoolPatchForm {
    fn default() -> Self {
        Self {
            mode: SeoBulkFieldPatchMode::Keep,
            value: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SeoBulkJsonPatchForm {
    pub mode: SeoBulkFieldPatchMode,
    pub value: String,
}

impl Default for SeoBulkJsonPatchForm {
    fn default() -> Self {
        Self {
            mode: SeoBulkFieldPatchMode::Keep,
            value: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SeoBulkActionForm {
    pub selection_mode: SeoBulkSelectionMode,
    pub apply_mode: SeoBulkApplyMode,
    pub publish_after_write: bool,
    pub import_csv: String,
    pub title: SeoBulkStringPatchForm,
    pub description: SeoBulkStringPatchForm,
    pub keywords: SeoBulkStringPatchForm,
    pub canonical_url: SeoBulkStringPatchForm,
    pub og_title: SeoBulkStringPatchForm,
    pub og_description: SeoBulkStringPatchForm,
    pub og_image: SeoBulkStringPatchForm,
    pub structured_data: SeoBulkJsonPatchForm,
    pub noindex: SeoBulkBoolPatchForm,
    pub nofollow: SeoBulkBoolPatchForm,
}

impl Default for SeoBulkActionForm {
    fn default() -> Self {
        Self {
            selection_mode: SeoBulkSelectionMode::CurrentFilterScope,
            apply_mode: SeoBulkApplyMode::ApplyMissingOnly,
            publish_after_write: true,
            import_csv: String::new(),
            title: SeoBulkStringPatchForm::default(),
            description: SeoBulkStringPatchForm::default(),
            keywords: SeoBulkStringPatchForm::default(),
            canonical_url: SeoBulkStringPatchForm::default(),
            og_title: SeoBulkStringPatchForm::default(),
            og_description: SeoBulkStringPatchForm::default(),
            og_image: SeoBulkStringPatchForm::default(),
            structured_data: SeoBulkJsonPatchForm::default(),
            noindex: SeoBulkBoolPatchForm::default(),
            nofollow: SeoBulkBoolPatchForm::default(),
        }
    }
}

impl SeoBulkActionForm {
    pub fn build_selection(
        &self,
        filter: SeoBulkListInput,
        selected_ids: &[Uuid],
    ) -> SeoBulkSelectionInput {
        SeoBulkSelectionInput {
            mode: self.selection_mode,
            selected_ids: selected_ids.to_vec(),
            filter: Some(filter),
        }
    }

    pub fn build_apply_input(
        &self,
        filter: SeoBulkListInput,
        selected_ids: &[Uuid],
    ) -> Result<SeoBulkApplyInput, String> {
        Ok(SeoBulkApplyInput {
            selection: self.build_selection(filter, selected_ids),
            patch: SeoBulkMetaPatchInput {
                title: Some(self.title.build_patch()?),
                description: Some(self.description.build_patch()?),
                keywords: Some(self.keywords.build_patch()?),
                canonical_url: Some(self.canonical_url.build_patch()?),
                og_title: Some(self.og_title.build_patch()?),
                og_description: Some(self.og_description.build_patch()?),
                og_image: Some(self.og_image.build_patch()?),
                structured_data: Some(self.structured_data.build_patch()?),
                noindex: Some(self.noindex.build_patch()),
                nofollow: Some(self.nofollow.build_patch()),
            },
            apply_mode: self.apply_mode,
            publish_after_write: self.publish_after_write
                && self.apply_mode != SeoBulkApplyMode::PreviewOnly,
        })
    }

    pub fn build_export_input(&self, filter: SeoBulkListInput) -> SeoBulkExportInput {
        SeoBulkExportInput { filter }
    }

    pub fn build_import_input(
        &self,
        filter: &SeoBulkListInput,
    ) -> Result<SeoBulkImportInput, String> {
        let csv_utf8 = self.import_csv.trim();
        if csv_utf8.is_empty() {
            return Err("CSV import payload is required".to_string());
        }

        Ok(SeoBulkImportInput {
            target_kind: filter.target_kind.clone(),
            locale: filter.locale.clone(),
            csv_utf8: csv_utf8.to_string(),
            publish_after_write: self.publish_after_write,
        })
    }
}

impl SeoBulkStringPatchForm {
    pub fn build_patch(&self) -> Result<SeoBulkStringFieldPatch, String> {
        if matches!(self.mode, SeoBulkFieldPatchMode::Set) && self.value.trim().is_empty() {
            return Err("String patch value is required for `set` mode".to_string());
        }
        Ok(SeoBulkStringFieldPatch {
            mode: self.mode,
            value: trim_to_option(self.value.as_str()),
        })
    }
}

impl SeoBulkBoolPatchForm {
    pub fn build_patch(&self) -> SeoBulkBoolFieldPatch {
        SeoBulkBoolFieldPatch {
            mode: self.mode,
            value: Some(self.value),
        }
    }
}

impl SeoBulkJsonPatchForm {
    pub fn build_patch(&self) -> Result<SeoBulkJsonFieldPatch, String> {
        let value = match self.mode {
            SeoBulkFieldPatchMode::Set => {
                let raw = self.value.trim();
                if raw.is_empty() {
                    return Err("Structured data JSON is required for `set` mode".to_string());
                }
                Some(Json(serde_json::from_str::<Value>(raw).map_err(|err| {
                    format!("Invalid structured data JSON: {err}")
                })?))
            }
            _ => None,
        };

        Ok(SeoBulkJsonFieldPatch {
            mode: self.mode,
            value,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SeoRedirectForm {
    pub match_type: SeoRedirectMatchType,
    pub source_pattern: String,
    pub target_url: String,
    pub status_code: String,
}

impl Default for SeoRedirectForm {
    fn default() -> Self {
        Self {
            match_type: SeoRedirectMatchType::Exact,
            source_pattern: String::new(),
            target_url: String::new(),
            status_code: "308".to_string(),
        }
    }
}

impl SeoRedirectForm {
    pub fn match_type_value(&self) -> &'static str {
        self.match_type.as_str()
    }

    pub fn set_match_type_from_str(&mut self, value: &str) {
        self.match_type = SeoRedirectMatchType::parse(value).unwrap_or(SeoRedirectMatchType::Exact);
    }

    pub fn build_input(&self) -> Result<SeoRedirectInput, String> {
        let status_code = self
            .status_code
            .trim()
            .parse::<i32>()
            .map_err(|_| "Invalid redirect status code".to_string())?;

        Ok(SeoRedirectInput {
            id: None,
            match_type: self.match_type,
            source_pattern: self.source_pattern.clone(),
            target_url: self.target_url.clone(),
            status_code,
            expires_at: None,
            is_active: true,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct SeoSettingsForm {
    pub default_robots: Vec<String>,
    pub robot_directive_input: String,
    pub sitemap_enabled: bool,
    pub allowed_redirect_hosts_text: String,
    pub allowed_canonical_hosts_text: String,
    pub x_default_locale: String,
    pub template_title: String,
    pub template_meta_description: String,
    pub template_canonical_url: String,
    pub template_keywords: String,
    pub template_robots: String,
    pub template_open_graph_title: String,
    pub template_open_graph_description: String,
    pub template_twitter_title: String,
    pub template_twitter_description: String,
    pub template_overrides_json: String,
}

impl SeoSettingsForm {
    pub fn from_settings(settings: &SeoModuleSettings) -> Self {
        Self {
            default_robots: settings.default_robots.clone(),
            robot_directive_input: String::new(),
            sitemap_enabled: settings.sitemap_enabled,
            allowed_redirect_hosts_text: settings.allowed_redirect_hosts.join("\n"),
            allowed_canonical_hosts_text: settings.allowed_canonical_hosts.join("\n"),
            x_default_locale: settings.x_default_locale.clone().unwrap_or_default(),
            template_title: settings.template_defaults.title.clone().unwrap_or_default(),
            template_meta_description: settings
                .template_defaults
                .meta_description
                .clone()
                .unwrap_or_default(),
            template_canonical_url: settings
                .template_defaults
                .canonical_url
                .clone()
                .unwrap_or_default(),
            template_keywords: settings
                .template_defaults
                .keywords
                .clone()
                .unwrap_or_default(),
            template_robots: settings
                .template_defaults
                .robots
                .clone()
                .unwrap_or_default(),
            template_open_graph_title: settings
                .template_defaults
                .open_graph_title
                .clone()
                .unwrap_or_default(),
            template_open_graph_description: settings
                .template_defaults
                .open_graph_description
                .clone()
                .unwrap_or_default(),
            template_twitter_title: settings
                .template_defaults
                .twitter_title
                .clone()
                .unwrap_or_default(),
            template_twitter_description: settings
                .template_defaults
                .twitter_description
                .clone()
                .unwrap_or_default(),
            template_overrides_json: serde_json::to_string_pretty(&settings.template_overrides)
                .unwrap_or_else(|_| "{}".to_string()),
        }
    }

    pub fn add_robot_directive(&mut self, value: String) {
        let directive = value.trim().to_ascii_lowercase();
        if directive.is_empty() {
            self.robot_directive_input.clear();
            return;
        }

        if !self
            .default_robots
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&directive))
        {
            self.default_robots.push(directive);
        }
        self.robot_directive_input.clear();
    }

    pub fn remove_robot_directive(&mut self, directive: &str) {
        self.default_robots
            .retain(|item| !item.eq_ignore_ascii_case(directive));
    }

    pub fn build_settings(&self) -> SeoModuleSettings {
        SeoModuleSettings {
            default_robots: normalize_robot_directives(self.default_robots.as_slice()),
            sitemap_enabled: self.sitemap_enabled,
            allowed_redirect_hosts: normalize_multiline_values(
                self.allowed_redirect_hosts_text.as_str(),
                true,
            ),
            allowed_canonical_hosts: normalize_multiline_values(
                self.allowed_canonical_hosts_text.as_str(),
                true,
            ),
            x_default_locale: trim_to_option(self.x_default_locale.as_str()),
            template_defaults: SeoTemplateRuleSet {
                title: trim_to_option(self.template_title.as_str()),
                meta_description: trim_to_option(self.template_meta_description.as_str()),
                canonical_url: trim_to_option(self.template_canonical_url.as_str()),
                keywords: trim_to_option(self.template_keywords.as_str()),
                robots: trim_to_option(self.template_robots.as_str()),
                open_graph_title: trim_to_option(self.template_open_graph_title.as_str()),
                open_graph_description: trim_to_option(
                    self.template_open_graph_description.as_str(),
                ),
                twitter_title: trim_to_option(self.template_twitter_title.as_str()),
                twitter_description: trim_to_option(self.template_twitter_description.as_str()),
            },
            template_overrides: parse_template_overrides(self.template_overrides_json.as_str()),
        }
    }
}

fn parse_template_overrides(value: &str) -> BTreeMap<String, SeoTemplateRuleSet> {
    serde_json::from_str::<BTreeMap<String, SeoTemplateRuleSet>>(value.trim()).unwrap_or_default()
}

fn normalize_robot_directives(values: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let directive = value.trim().to_ascii_lowercase();
        if directive.is_empty()
            || normalized
                .iter()
                .any(|item: &String| item.eq_ignore_ascii_case(&directive))
        {
            continue;
        }
        normalized.push(directive);
    }
    normalized
}

fn normalize_multiline_values(value: &str, lowercase: bool) -> Vec<String> {
    let mut normalized = Vec::new();
    for line in value.lines() {
        let item = line.trim();
        if item.is_empty() {
            continue;
        }

        let item = if lowercase {
            item.to_ascii_lowercase()
        } else {
            item.to_string()
        };
        if normalized.iter().any(|existing| existing == &item) {
            continue;
        }
        normalized.push(item);
    }
    normalized
}

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{SeoAdminTab, SeoBulkActionForm, SeoBulkFilterForm, SeoSettingsForm};
    use rustok_seo::{
        seo_builtin_slug, SeoBulkApplyMode, SeoBulkFieldPatchMode, SeoModuleSettings, SeoTargetSlug,
    };

    #[test]
    fn seo_admin_tab_roundtrip_covers_control_plane_tabs() {
        assert_eq!(
            SeoAdminTab::from_str(SeoAdminTab::Bulk.as_str()),
            Some(SeoAdminTab::Bulk)
        );
        assert_eq!(
            SeoAdminTab::from_str(SeoAdminTab::Redirects.as_str()),
            Some(SeoAdminTab::Redirects)
        );
        assert_eq!(
            SeoAdminTab::from_str(SeoAdminTab::Sitemaps.as_str()),
            Some(SeoAdminTab::Sitemaps)
        );
        assert_eq!(
            SeoAdminTab::from_str(SeoAdminTab::Robots.as_str()),
            Some(SeoAdminTab::Robots)
        );
        assert_eq!(
            SeoAdminTab::from_str(SeoAdminTab::Defaults.as_str()),
            Some(SeoAdminTab::Defaults)
        );
        assert_eq!(
            SeoAdminTab::from_str(SeoAdminTab::Diagnostics.as_str()),
            Some(SeoAdminTab::Diagnostics)
        );
    }

    #[test]
    fn settings_form_builds_trimmed_settings_payload() {
        let mut form = SeoSettingsForm::from_settings(&SeoModuleSettings::default());
        form.default_robots = vec![
            "Index".to_string(),
            " follow ".to_string(),
            "INDEX".to_string(),
            String::new(),
        ];
        form.allowed_redirect_hosts_text =
            " Example.com \nexample.com\ncdn.example.com\n".to_string();
        form.allowed_canonical_hosts_text = " Blog.Example.com \n".to_string();
        form.x_default_locale = " en-US ".to_string();

        let settings = form.build_settings();
        assert_eq!(settings.default_robots, vec!["index", "follow"]);
        assert_eq!(
            settings.allowed_redirect_hosts,
            vec!["example.com", "cdn.example.com"]
        );
        assert_eq!(settings.allowed_canonical_hosts, vec!["blog.example.com"]);
        assert_eq!(settings.x_default_locale.as_deref(), Some("en-US"));
    }

    #[test]
    fn bulk_filter_form_builds_single_scope_input() {
        let mut form = SeoBulkFilterForm::new(Some("ru-RU"));
        form.target_kind =
            SeoTargetSlug::new(seo_builtin_slug::BLOG_POST).expect("builtin SEO target slug");
        form.locale = " en-us ".to_string();
        form.query = " Sale ".to_string();
        form.per_page = 500;

        let input = form.build_input().expect("build bulk filter");
        assert_eq!(
            input.target_kind,
            SeoTargetSlug::new(seo_builtin_slug::BLOG_POST).expect("builtin SEO target slug")
        );
        assert_eq!(input.locale, "en-us");
        assert_eq!(input.query.as_deref(), Some("Sale"));
        assert_eq!(input.per_page, 100);
    }

    #[test]
    fn bulk_action_form_validates_json_patch() {
        let mut form = SeoBulkActionForm::default();
        form.structured_data.mode = SeoBulkFieldPatchMode::Set;
        form.structured_data.value = "{".to_string();

        assert!(form
            .structured_data
            .build_patch()
            .expect_err("invalid json must fail")
            .contains("Invalid structured data JSON"));
    }

    #[test]
    fn bulk_action_form_defaults_to_missing_only_remediation() {
        let form = SeoBulkActionForm::default();

        assert_eq!(form.apply_mode, SeoBulkApplyMode::ApplyMissingOnly);
    }

    #[test]
    fn preview_bulk_action_never_requests_publish() {
        let form = SeoBulkActionForm {
            apply_mode: SeoBulkApplyMode::PreviewOnly,
            publish_after_write: true,
            ..SeoBulkActionForm::default()
        };
        let filter = SeoBulkFilterForm::new(Some("en"))
            .build_input()
            .expect("filter input");

        let input = form.build_apply_input(filter, &[]).expect("apply input");

        assert!(!input.publish_after_write);
    }
}
