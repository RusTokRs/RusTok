use rustok_revisions::{RetentionPolicy, RevisionEvent};
use std::collections::HashMap;

/// Configuration for content revision tracking across RusTok modules.
#[derive(Debug, Clone)]
pub struct ContentRevisionConfig {
    /// Global enabled flag (can be overridden per content type)
    pub enabled: bool,

    /// Configuration per content type
    pub content_types: HashMap<String, ContentTypeConfig>,

    /// Default retention policy for new content types
    pub default_retention: RetentionPolicy,

    /// Default events to track
    pub default_track_on: Vec<RevisionEvent>,
}

impl Default for ContentRevisionConfig {
    fn default() -> Self {
        let mut config = Self {
            enabled: true,
            content_types: HashMap::new(),
            default_retention: RetentionPolicy::KeepLast(50),
            default_track_on: vec![RevisionEvent::Update],
        };

        // Pre-configure known content types
        config.configure_blog();
        config.configure_forum();
        config.configure_commerce();

        config
    }
}

impl ContentRevisionConfig {
    /// Create a new empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure blog post revisions.
    fn configure_blog(&mut self) {
        self.content_types.insert(
            "blog_post".to_string(),
            ContentTypeConfig {
                enabled: true,
                track_on: vec![RevisionEvent::Update],
                condition_field: Some("status".to_string()),
                condition_value: Some("published".to_string()),
                retention: RetentionPolicy::KeepLast(50),
                tracked_fields: vec![
                    "title".to_string(),
                    "content".to_string(),
                    "status".to_string(),
                ],
                ignored_fields: vec![
                    "updated_at".to_string(),
                    "view_count".to_string(),
                ],
            },
        );

        self.content_types.insert(
            "blog_category".to_string(),
            ContentTypeConfig {
                enabled: false, // Categories don't need revision history
                track_on: vec![],
                condition_field: None,
                condition_value: None,
                retention: RetentionPolicy::KeepAll,
                tracked_fields: vec![],
                ignored_fields: vec![],
            },
        );
    }

    /// Configure forum revisions.
    fn configure_forum(&mut self) {
        // Forum topics - disabled by default (can be enabled for wiki mode)
        self.content_types.insert(
            "forum_topic".to_string(),
            ContentTypeConfig {
                enabled: false,
                track_on: vec![RevisionEvent::Update],
                condition_field: None,
                condition_value: None,
                retention: RetentionPolicy::KeepLast(20),
                tracked_fields: vec![
                    "title".to_string(),
                    "content".to_string(),
                ],
                ignored_fields: vec![
                    "updated_at".to_string(),
                    "reply_count".to_string(),
                    "view_count".to_string(),
                ],
            },
        );

        // Forum posts - disabled
        self.content_types.insert(
            "forum_post".to_string(),
            ContentTypeConfig {
                enabled: false,
                track_on: vec![],
                condition_field: None,
                condition_value: None,
                retention: RetentionPolicy::KeepAll,
                tracked_fields: vec![],
                ignored_fields: vec![],
            },
        );
    }

    /// Configure commerce revisions.
    fn configure_commerce(&mut self) {
        // Products - disabled by default
        self.content_types.insert(
            "product".to_string(),
            ContentTypeConfig {
                enabled: false,
                track_on: vec![RevisionEvent::Update],
                condition_field: None,
                condition_value: None,
                retention: RetentionPolicy::KeepLast(30),
                tracked_fields: vec![
                    "name".to_string(),
                    "description".to_string(),
                    "price".to_string(),
                ],
                ignored_fields: vec![
                    "updated_at".to_string(),
                    "stock".to_string(),
                    "view_count".to_string(),
                ],
            },
        );
    }

    /// Get configuration for a content type.
    pub fn get_content_type(&self, content_type: &str) -> Option<&ContentTypeConfig> {
        self.content_types.get(content_type)
    }

    /// Check if tracking is enabled for a content type.
    pub fn is_enabled(&self, content_type: &str) -> bool {
        if !self.enabled {
            return false;
        }

        self.content_types
            .get(content_type)
            .map(|c| c.enabled)
            .unwrap_or(false)
    }

    /// Add or update configuration for a content type.
    pub fn set_content_type(&mut self, content_type: String, config: ContentTypeConfig) {
        self.content_types.insert(content_type, config);
    }
}

/// Configuration for a specific content type.
#[derive(Debug, Clone)]
pub struct ContentTypeConfig {
    /// Whether tracking is enabled for this content type
    pub enabled: bool,

    /// Events that trigger revision creation
    pub track_on: Vec<RevisionEvent>,

    /// Optional condition field (e.g., "status")
    pub condition_field: Option<String>,

    /// Optional condition value (e.g., "published")
    pub condition_value: Option<String>,

    /// Retention policy for this content type
    pub retention: RetentionPolicy,

    /// Fields to track for changes
    pub tracked_fields: Vec<String>,

    /// Fields to ignore (never track)
    pub ignored_fields: Vec<String>,
}

impl ContentTypeConfig {
    /// Create a new content type configuration.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            track_on: vec![RevisionEvent::Update],
            condition_field: None,
            condition_value: None,
            retention: RetentionPolicy::KeepLast(50),
            tracked_fields: vec![],
            ignored_fields: vec![],
        }
    }

    /// Set events to track.
    pub fn track_on(mut self, events: Vec<RevisionEvent>) -> Self {
        self.track_on = events;
        self
    }

    /// Set condition for tracking.
    pub fn condition(mut self, field: &str, value: &str) -> Self {
        self.condition_field = Some(field.to_string());
        self.condition_value = Some(value.to_string());
        self
    }

    /// Set retention policy.
    pub fn retention(mut self, policy: RetentionPolicy) -> Self {
        self.retention = policy;
        self
    }

    /// Set tracked fields.
    pub fn tracked_fields(mut self, fields: Vec<&str>) -> Self {
        self.tracked_fields = fields.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set ignored fields.
    pub fn ignored_fields(mut self, fields: Vec<&str>) -> Self {
        self.ignored_fields = fields.into_iter().map(|s| s.to_string()).collect();
        self
    }
}
