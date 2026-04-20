use rustok_seo::{SeoModuleSettings, SeoRedirectInput, SeoRedirectMatchType};

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
    Redirects,
    Sitemaps,
    Robots,
    Defaults,
    Diagnostics,
}

impl SeoAdminTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Redirects => "redirects",
            Self::Sitemaps => "sitemaps",
            Self::Robots => "robots",
            Self::Defaults => "defaults",
            Self::Diagnostics => "diagnostics",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
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
        self.match_type =
            SeoRedirectMatchType::from_str(value).unwrap_or(SeoRedirectMatchType::Exact);
    }

    pub fn build_input(&self) -> Result<SeoRedirectInput, String> {
        let status_code = self
            .status_code
            .trim()
            .parse::<i32>()
            .map_err(|_| "Invalid redirect status code".to_string())?;

        Ok(SeoRedirectInput {
            id: None,
            match_type: self.match_type.clone(),
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
        }
    }
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
    use super::{SeoAdminTab, SeoSettingsForm};
    use rustok_seo::SeoModuleSettings;

    #[test]
    fn seo_admin_tab_roundtrip_covers_control_plane_tabs() {
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
}
