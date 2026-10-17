use crate::model::{
    CreateStorefrontForumReplyDraft, CreateStorefrontForumTopicDraft, StorefrontForumComposerQuote,
};
use rustok_ui_core::css_hex_accent_class;
use uuid::Uuid;

const FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12;
const MAX_FORUM_ROUTE_LOCALE_LEN: usize = 64;
const MAX_FORUM_ROUTE_SLUG_LEN: usize = 255;

/// Copy the category rail cards need. Counts are not here on purpose: the surface renders them
/// from `ForumStorefrontCategoryCardViewModel::topic_count`, so a template carried through the
/// builder would be dead weight that looks like a configured label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontCategoryRailLabels {
    pub no_description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontCategoryCardViewModel {
    pub href: String,
    pub is_active: bool,
    pub container_class: &'static str,
    pub accent_class: &'static str,
    pub name: String,
    pub slug_badge: String,
    pub topic_count: i32,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumStorefrontTopicCardViewModel {
    pub href: String,
    pub is_active: bool,
    pub container_class: &'static str,
    pub status_class: &'static str,
    pub status_badge_class: &'static str,
    pub unread_badge_class: &'static str,
    pub status: String,
    pub effective_locale: String,
    pub is_pinned: bool,
    pub is_locked: bool,
    pub is_unread: bool,
    pub unread_count: i64,
    pub has_unread_topic_revision: bool,
    pub title: String,
    pub slug_label: String,
    pub reply_count: i32,
    pub is_solved: bool,
    pub vote_score: i32,
}

pub fn forum_storefront_count_label(template: &str, count: impl ToString) -> String {
    template.replace("{count}", count.to_string().as_str())
}

pub fn forum_storefront_slug_label(template: &str, slug: &str) -> String {
    template.replace("{slug}", slug)
}

pub fn forum_storefront_category_card_class(is_active: bool) -> &'static str {
    if is_active {
        "border-primary/40 bg-primary/5 shadow-sm"
    } else {
        "border-border bg-background hover:border-primary/20 hover:bg-muted/40"
    }
}

pub fn forum_storefront_topic_card_class(is_active: bool, is_unread: bool) -> &'static str {
    if is_active {
        "border-primary/40 bg-primary/5 shadow-sm"
    } else if is_unread {
        "border-sky-500/40 bg-sky-500/5 hover:border-sky-500/60 hover:shadow-sm"
    } else {
        "border-border bg-background hover:border-primary/25 hover:shadow-sm"
    }
}

pub fn forum_storefront_accent_class(color: Option<&str>) -> &'static str {
    css_hex_accent_class(color)
}

pub fn forum_storefront_status_badge_class(status_class: &'static str) -> &'static str {
    match status_class {
        "success" => {
            "rounded-full bg-emerald-500/15 px-2.5 py-1 text-[11px] font-medium text-emerald-700 dark:text-emerald-300"
        }
        "warning" => {
            "rounded-full bg-amber-500/15 px-2.5 py-1 text-[11px] font-medium text-amber-700 dark:text-amber-300"
        }
        "muted" => {
            "rounded-full bg-muted px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
        _ => {
            "rounded-full border border-border px-2.5 py-1 text-[11px] font-medium text-muted-foreground"
        }
    }
}

pub fn forum_storefront_unread_badge_class() -> &'static str {
    "rounded-full bg-sky-500/15 px-2.5 py-1 text-[11px] font-semibold text-sky-700 dark:text-sky-300"
}

pub fn forum_storefront_category_card_view_model(
    module_route_base: &str,
    item: &crate::model::ForumCategoryListItem,
    selected_category_id: Option<&str>,
    labels: &ForumStorefrontCategoryRailLabels,
) -> ForumStorefrontCategoryCardViewModel {
    let is_active = selected_category_id == Some(item.id.as_str());
    ForumStorefrontCategoryCardViewModel {
        href: category_href(item.effective_locale.as_str(), item.slug.as_str())
            .unwrap_or_else(|| module_route_base.to_string()),
        is_active,
        container_class: forum_storefront_category_card_class(is_active),
        accent_class: forum_storefront_accent_class(item.color.as_deref()),
        name: item.name.clone(),
        slug_badge: forum_storefront_slug_label("#{slug}", item.slug.as_str()),
        topic_count: item.topic_count,
        description: item
            .description
            .clone()
            .unwrap_or_else(|| labels.no_description.clone()),
    }
}

pub fn forum_storefront_topic_card_view_model(
    module_route_base: &str,
    item: &crate::model::ForumTopicListItem,
    _selected_category_id: Option<&str>,
    selected_topic_id: Option<&str>,
    slug_label: &str,
) -> ForumStorefrontTopicCardViewModel {
    let is_active = selected_topic_id == Some(item.id.as_str());
    let is_unread = item.is_unread.unwrap_or(false);
    let status_class = topic_status_class(item.status.as_str());
    ForumStorefrontTopicCardViewModel {
        href: topic_href(
            item.id.as_str(),
            item.effective_locale.as_str(),
            item.slug.as_str(),
        )
        .unwrap_or_else(|| module_route_base.to_string()),
        is_active,
        container_class: forum_storefront_topic_card_class(is_active, is_unread),
        status_class,
        status_badge_class: forum_storefront_status_badge_class(status_class),
        unread_badge_class: forum_storefront_unread_badge_class(),
        status: item.status.clone(),
        effective_locale: item.effective_locale.clone(),
        is_pinned: item.is_pinned,
        is_locked: item.is_locked,
        is_unread,
        unread_count: item.unread_count.unwrap_or(0),
        has_unread_topic_revision: item.has_unread_topic_revision.unwrap_or(false),
        title: item.title.clone(),
        slug_label: slug_label.to_string(),
        reply_count: item.reply_count,
        is_solved: item.solution_reply_id.is_some(),
        vote_score: item.vote_score.unwrap_or(0),
    }
}

pub fn category_href(locale: &str, slug: &str) -> Option<String> {
    let locale = normalize_route_locale(locale)?;
    let slug = normalize_route_slug(slug)?;
    Some(format!("/{locale}/forum/c/{slug}"))
}

pub fn topic_href(topic_id: &str, locale: &str, slug: &str) -> Option<String> {
    let topic_id = Uuid::parse_str(topic_id.trim()).ok()?;
    let locale = normalize_route_locale(locale)?;
    let slug = normalize_route_slug(slug)?;
    let compact = topic_id.simple().to_string();
    let short_id = &compact[..FORUM_TOPIC_ROUTE_SHORT_ID_LEN];
    Some(format!("/{locale}/forum/t/{short_id}/{slug}"))
}

fn normalize_route_locale(value: &str) -> Option<String> {
    let locale = rustok_api::normalize_locale_tag(value.trim())?;
    (locale.chars().count() <= MAX_FORUM_ROUTE_LOCALE_LEN).then_some(locale)
}

fn normalize_route_slug(value: &str) -> Option<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    (!normalized.is_empty() && normalized.len() <= MAX_FORUM_ROUTE_SLUG_LEN).then_some(normalized)
}

pub fn topic_status_class(status: &str) -> &'static str {
    match status.to_ascii_lowercase().as_str() {
        "published" | "active" | "open" | "approved" => "success",
        "draft" | "pending" => "warning",
        "archived" | "closed" | "hidden" => "muted",
        _ => "default",
    }
}

/// Content locale a storefront composer write is authored in.
///
/// The locale comes from the host route context only; when the host publishes
/// none, the platform fallback applies, which is the same resolution the owner
/// read paths use.
pub fn forum_storefront_content_locale(locale: Option<&str>) -> String {
    locale
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE)
        .to_string()
}

/// Normalizes a composer quote target kind to the owner quote vocabulary.
pub fn forum_storefront_quote_target_kind(kind: &str) -> Option<&'static str> {
    match kind.trim().to_ascii_uppercase().as_str() {
        "TOPIC" => Some("TOPIC"),
        "REPLY" => Some("REPLY"),
        _ => None,
    }
}

/// A reply target only accepts an inline quote when the author pinned the exact
/// quoted revision, so an unresolved revision must fail closed in the composer
/// instead of inventing a revision number.
pub fn forum_storefront_quote_revision(revision_id: Option<i64>) -> Option<i64> {
    revision_id.filter(|value| *value > 0)
}

/// True when the composer text carries something the owner can store. An empty
/// discussion document is rejected by the owner contract, so the composer keeps
/// the submit action disabled rather than sending a doomed write.
pub fn forum_storefront_composer_has_text(text: &str) -> bool {
    text.chars().any(|character| !character.is_whitespace())
}

/// Whether the storefront composer may post into this topic.
///
/// The owner accepts replies only into an open, unlocked topic (`TopicStatus::Open`
/// plus the `locked` flag, per `state_machine.rs`), and the topic detail already
/// carries both signals, so the affordance is hidden instead of failing after the
/// author pressed submit.
pub fn forum_storefront_topic_accepts_reply(status: &str, is_locked: bool) -> bool {
    !is_locked && status.eq_ignore_ascii_case("open")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_gates_reply_on_topic_state() {
        assert!(forum_storefront_topic_accepts_reply("open", false));
        assert!(forum_storefront_topic_accepts_reply("OPEN", false));
        assert!(!forum_storefront_topic_accepts_reply("open", true));
        assert!(!forum_storefront_topic_accepts_reply("closed", false));
        assert!(!forum_storefront_topic_accepts_reply("archived", false));
        assert!(!forum_storefront_topic_accepts_reply("pending", false));
        assert_eq!(forum_storefront_quote_revision(Some(7)), Some(7));
        assert_eq!(forum_storefront_quote_revision(Some(0)), None);
        assert_eq!(forum_storefront_quote_revision(None), None);
        assert!(forum_storefront_composer_has_text(" a "));
        assert!(!forum_storefront_composer_has_text("   \n "));
    }

    #[test]
    fn builds_canonical_category_and_topic_hrefs() {
        assert_eq!(
            category_href("en-us", "General Discussion").as_deref(),
            Some("/en-US/forum/c/general-discussion")
        );
        assert_eq!(
            topic_href(
                "12345678-9abc-4def-8123-456789abcdef",
                "en-us",
                "Welcome Thread"
            )
            .as_deref(),
            Some("/en-US/forum/t/123456789abc/welcome-thread")
        );
    }

    #[test]
    fn canonical_hrefs_fail_closed_for_invalid_identity_locale_or_slug() {
        assert_eq!(category_href("invalid_locale!", "general"), None);
        assert_eq!(category_href("en", "---"), None);
        assert_eq!(topic_href("topic-1", "en", "welcome"), None);
        assert_eq!(
            topic_href("12345678-9abc-4def-8123-456789abcdef", "en", "---"),
            None
        );
    }

    #[test]
    fn maps_topic_status_to_stable_badge_class_keys() {
        assert_eq!(topic_status_class("PUBLISHED"), "success");
        assert_eq!(topic_status_class("pending"), "warning");
        assert_eq!(topic_status_class("closed"), "muted");
        assert_eq!(topic_status_class("unknown"), "default");
    }

    #[test]
    fn builds_storefront_copy_and_class_policies_without_framework_state() {
        assert_eq!(
            forum_storefront_count_label("{count} threads", 12_u64),
            "12 threads"
        );
        assert_eq!(
            forum_storefront_slug_label("slug: {slug}", "welcome"),
            "slug: welcome"
        );
        assert!(forum_storefront_category_card_class(true).contains("border-primary/40"));
        assert!(forum_storefront_topic_card_class(false, false).contains("hover:shadow-sm"));
        assert!(forum_storefront_topic_card_class(false, true).contains("border-sky-500/40"));
        assert_eq!(forum_storefront_accent_class(Some("#0ea5e9")), "bg-sky-500");
        assert!(forum_storefront_accent_class(Some(" ")).contains("from-sky-500"));
        assert!(forum_storefront_status_badge_class("success").contains("emerald"));
        assert!(forum_storefront_status_badge_class("warning").contains("amber"));
        assert!(forum_storefront_status_badge_class("muted").contains("bg-muted"));
        assert!(forum_storefront_status_badge_class("default").contains("border-border"));
        assert!(forum_storefront_unread_badge_class().contains("sky"));
    }

    #[test]
    fn rejects_persisted_css_declaration_injection() {
        let class =
            forum_storefront_accent_class(Some("#fff;background:url(https://attacker.invalid/x)"));
        assert!(class.contains("from-sky-500"));
    }
}

/// A composer quote. `revision_id` pins the exact quoted source revision when the
/// caller already knows it; `None` keeps the quote as quoted text only, because
/// the owner contract rejects a fabricated revision.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComposerQuote {
    pub target_kind: String,
    pub target_id: String,
    pub revision_id: Option<i64>,
    pub author_handle: Option<String>,
    pub snippet: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ComposerMode {
    #[default]
    Reply,
    Topic,
}

#[derive(Clone, Debug, Default)]
pub struct ComposerState {
    pub is_open: bool,
    pub is_minimized: bool,
    pub mode: ComposerMode,
    pub topic_id: Option<String>,
    pub topic_title: Option<String>,
    pub category_id: Option<String>,
    pub parent_reply_id: Option<String>,
    pub title: String,
    pub content: String,
    pub quotes: Vec<ComposerQuote>,
    pub is_submitting: bool,
    pub error: Option<String>,
    drafts: std::collections::HashMap<ComposerTarget, ComposerDraft>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum ComposerTarget {
    Reply(String, Option<String>),
    Topic(Option<String>),
}

#[derive(Clone, Debug, Default)]
struct ComposerDraft {
    title: String,
    content: String,
    quotes: Vec<ComposerQuote>,
}

pub(crate) enum ComposerSubmission {
    Topic(CreateStorefrontForumTopicDraft),
    Reply(CreateStorefrontForumReplyDraft),
}

impl ComposerState {
    fn target(&self) -> ComposerTarget {
        match self.mode {
            ComposerMode::Reply => ComposerTarget::Reply(
                self.topic_id.clone().unwrap_or_default(),
                self.parent_reply_id.clone(),
            ),
            ComposerMode::Topic => ComposerTarget::Topic(self.category_id.clone()),
        }
    }

    fn save_draft(&mut self) {
        self.drafts.insert(
            self.target(),
            ComposerDraft {
                title: self.title.clone(),
                content: self.content.clone(),
                quotes: self.quotes.clone(),
            },
        );
    }

    fn restore_draft(&mut self) {
        let draft = self.drafts.remove(&self.target()).unwrap_or_default();
        self.title = draft.title;
        self.content = draft.content;
        self.quotes = draft.quotes;
        self.error = None;
    }

    pub fn select_category(&mut self, category_id: Option<String>) {
        if self.is_submitting || self.category_id == category_id {
            return;
        }
        let previous = self.target();
        self.save_draft();
        self.category_id = category_id;
        if self.drafts.contains_key(&self.target()) {
            self.restore_draft();
        } else {
            // Selecting an empty destination moves the current editor instead
            // of clearing text typed before choosing a posting category.
            self.drafts.remove(&previous);
            self.error = None;
        }
    }

    pub(crate) fn begin_submission(&mut self, locale: String) -> Option<ComposerSubmission> {
        if !self.can_submit() {
            return None;
        }
        let quotes = self.transport_quotes();
        let body_text = self.content.clone();
        let submission = match self.mode {
            ComposerMode::Topic => ComposerSubmission::Topic(CreateStorefrontForumTopicDraft {
                category_id: self.category_id.clone().unwrap_or_default(),
                title: self.title.trim().to_string(),
                body_text,
                locale,
                quotes,
            }),
            ComposerMode::Reply => ComposerSubmission::Reply(CreateStorefrontForumReplyDraft {
                topic_id: self.topic_id.clone().unwrap_or_default(),
                parent_reply_id: self.parent_reply_id.clone(),
                body_text,
                locale,
                quotes,
            }),
        };
        self.is_submitting = true;
        self.error = None;
        Some(submission)
    }

    pub fn fail_submission(&mut self, error: String) {
        self.is_submitting = false;
        self.error = Some(error);
    }

    pub fn open_reply(
        &mut self,
        topic_id: String,
        topic_title: String,
        parent_reply_id: Option<String>,
    ) {
        if self.is_submitting {
            return;
        }
        self.save_draft();
        self.mode = ComposerMode::Reply;
        self.category_id = None;
        self.topic_id = Some(topic_id);
        self.topic_title = Some(topic_title);
        self.parent_reply_id = parent_reply_id;
        self.restore_draft();
        self.is_open = true;
        self.is_minimized = false;
    }

    pub fn open_topic(&mut self, category_id: Option<String>) {
        if self.is_submitting {
            return;
        }
        self.save_draft();
        self.mode = ComposerMode::Topic;
        self.topic_id = None;
        self.topic_title = None;
        self.category_id = category_id;
        self.parent_reply_id = None;
        self.restore_draft();
        self.is_open = true;
        self.is_minimized = false;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.is_minimized = false;
    }

    pub fn minimize(&mut self) {
        self.is_minimized = true;
    }

    pub fn expand(&mut self) {
        self.is_minimized = false;
    }

    pub fn append_quote(&mut self, quote: ComposerQuote, quote_text: Option<String>) {
        if self.is_submitting {
            return;
        }
        if !self
            .quotes
            .iter()
            .any(|q| q.target_id == quote.target_id && q.target_kind == quote.target_kind)
        {
            self.quotes.push(quote.clone());
        }

        if let Some(text) = quote_text {
            let quote_header = match &quote.author_handle {
                Some(handle) => format!("> @{handle}:\n"),
                None => "> Quote:\n".to_string(),
            };
            let quoted_lines = text
                .lines()
                .map(|l| format!("> {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            let quote_block = format!("{quote_header}{quoted_lines}\n\n");
            self.content.push_str(&quote_block);
        }

        self.is_open = true;
        self.is_minimized = false;
    }

    pub fn remove_quote(&mut self, index: usize) {
        if !self.is_submitting && index < self.quotes.len() {
            self.quotes.remove(index);
        }
    }

    pub fn reset(&mut self) {
        self.drafts.remove(&self.target());
        self.title.clear();
        self.content.clear();
        self.quotes.clear();
        self.error = None;
        self.is_submitting = false;
        self.is_open = false;
        self.is_minimized = false;
    }

    /// Whether the owner contract can accept the current draft.
    ///
    /// A topic needs a title and a category and a reply needs its topic. The owner
    /// rejects an empty discussion document, so the submit action stays disabled
    /// instead of sending a doomed write.
    pub fn can_submit(&self) -> bool {
        if self.is_submitting || !forum_storefront_composer_has_text(self.content.as_str()) {
            return false;
        }
        match self.mode {
            ComposerMode::Topic => {
                !self.title.trim().is_empty()
                    && self
                        .category_id
                        .as_deref()
                        .is_some_and(|category_id| !category_id.trim().is_empty())
            }
            ComposerMode::Reply => self
                .topic_id
                .as_deref()
                .is_some_and(|topic_id| !topic_id.trim().is_empty()),
        }
    }

    /// Quotes that may be sent as owner relations, i.e. only the ones pinned to a
    /// positive source revision. Unpinned quotes stay in the post body as text.
    pub(crate) fn transport_quotes(&self) -> Vec<StorefrontForumComposerQuote> {
        self.quotes
            .iter()
            .filter_map(|quote| {
                forum_storefront_quote_revision(quote.revision_id).map(|revision_id| {
                    StorefrontForumComposerQuote {
                        target_kind: quote.target_kind.clone(),
                        target_id: quote.target_id.clone(),
                        revision_id,
                    }
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod composer_tests {
    use super::*;

    fn quote() -> ComposerQuote {
        ComposerQuote {
            target_kind: "REPLY".into(),
            target_id: "quoted".into(),
            revision_id: Some(7),
            snippet: Some("quoted text".into()),
            ..Default::default()
        }
    }

    #[test]
    fn switching_topic_and_mode_restores_each_draft_without_moving_text() {
        let mut state = ComposerState::default();
        state.open_reply("a".into(), "A".into(), None);
        state.content = "reply A".into();
        state.quotes.push(quote());
        state.open_reply("b".into(), "B".into(), None);
        assert!(state.content.is_empty());
        assert!(state.quotes.is_empty());
        state.content = "reply B".into();
        state.open_topic(Some("category".into()));
        assert!(state.content.is_empty());
        state.title = "New topic".into();
        state.content = "topic body".into();
        state.open_reply("a".into(), "A".into(), None);
        assert_eq!(state.content, "reply A");
        assert_eq!(state.quotes, vec![quote()]);
        state.open_reply("b".into(), "B".into(), None);
        assert_eq!(state.content, "reply B");
        state.open_topic(Some("category".into()));
        assert_eq!(state.title, "New topic");
        assert_eq!(state.content, "topic body");
    }

    #[test]
    fn reply_parent_and_category_are_part_of_the_draft_target() {
        let mut state = ComposerState::default();
        state.open_reply("a".into(), "A".into(), Some("parent-a".into()));
        state.content = "answer to parent A".into();
        state.open_reply("a".into(), "A".into(), Some("parent-b".into()));
        assert!(state.content.is_empty());
        state.open_reply("a".into(), "A".into(), Some("parent-a".into()));
        assert_eq!(state.content, "answer to parent A");
        state.open_topic(Some("category-a".into()));
        state.title = "A".into();
        state.content = "category A draft".into();
        state.open_topic(Some("category-b".into()));
        assert!(state.content.is_empty());
        state.title = "B".into();
        state.content = "category B draft".into();
        state.select_category(Some("category-a".into()));
        assert_eq!(state.content, "category A draft");
        assert_eq!(state.title, "A");
        state.select_category(Some("category-b".into()));
        assert_eq!(state.content, "category B draft");
    }

    #[test]
    fn submission_freezes_target_and_quotes_and_failure_preserves_the_draft() {
        let mut state = ComposerState::default();
        state.open_reply("a".into(), "A".into(), Some("parent".into()));
        state.content = "pending reply".into();
        state.quotes.push(quote());
        let Some(ComposerSubmission::Reply(request)) = state.begin_submission("ru".into()) else {
            panic!("a complete reply must produce its transport command");
        };
        assert_eq!(request.topic_id, "a");
        assert_eq!(request.parent_reply_id.as_deref(), Some("parent"));
        assert_eq!(request.body_text, "pending reply");
        assert_eq!(request.locale, "ru");
        assert_eq!(request.quotes[0].revision_id, 7);
        assert!(state.begin_submission("en".into()).is_none());
        state.close();
        state.open_reply("b".into(), "B".into(), None);
        state.open_topic(Some("other".into()));
        state.select_category(Some("other".into()));
        state.append_quote(quote(), Some("late quote".into()));
        state.remove_quote(0);
        assert_eq!(state.topic_id.as_deref(), Some("a"));
        assert_eq!(state.content, "pending reply");
        assert_eq!(state.quotes, vec![quote()]);
        assert!(!state.is_open);
        state.fail_submission("temporary failure".into());
        assert!(!state.is_submitting);
        assert_eq!(state.content, "pending reply");
        state.open_reply("a".into(), "A".into(), Some("parent".into()));
        assert_eq!(state.content, "pending reply");
        assert!(state.begin_submission("ru".into()).is_some());
    }

    #[test]
    fn success_discards_only_the_submitted_draft_and_does_not_resurrect_it() {
        let mut state = ComposerState::default();
        state.open_reply("b".into(), "B".into(), None);
        state.content = "keep B".into();
        state.open_reply("a".into(), "A".into(), None);
        state.content = "send A".into();
        assert!(state.begin_submission("en".into()).is_some());
        state.reset();
        state.open_reply("b".into(), "B".into(), None);
        assert_eq!(state.content, "keep B");
        state.open_reply("a".into(), "A".into(), None);
        assert!(state.content.is_empty());
    }

    #[test]
    fn choosing_an_empty_category_moves_the_current_text_without_erasing_it() {
        let mut state = ComposerState::default();
        state.open_topic(None);
        state.title = "draft title".into();
        state.content = "typed before selecting a category".into();
        state.select_category(Some("category".into()));
        assert_eq!(state.content, "typed before selecting a category");
        assert_eq!(state.title, "draft title");
        assert!(state.begin_submission("en".into()).is_some());
    }

    #[test]
    fn incomplete_drafts_do_not_start_a_write() {
        let mut state = ComposerState::default();
        assert!(state.begin_submission("en".into()).is_none());
        state.open_topic(None);
        state.content = "body".into();
        state.title = "title".into();
        assert!(state.begin_submission("en".into()).is_none());
        state.select_category(Some("category".into()));
        state.content = "body".into();
        state.title = "title".into();
        assert!(matches!(
            state.begin_submission("en".into()),
            Some(ComposerSubmission::Topic(_))
        ));
    }
}
