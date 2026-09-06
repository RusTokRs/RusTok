use serde::{Deserialize, Serialize};

pub const MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN: usize = 64;
pub const MAX_FORUM_TOPIC_ROUTE_SLUG_LEN: usize = 255;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSlugRenameCandidate {
    pub id: String,
    pub title: String,
    pub locale: String,
    pub slug: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSlugRenameCommand {
    pub topic_id: String,
    pub locale: String,
    pub slug: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicRouteDescriptor {
    pub topic_id: String,
    pub locale: String,
    pub short_id: String,
    pub slug: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForumTopicSlugRenameReceipt {
    pub topic_id: String,
    pub locale: String,
    pub previous_slug: String,
    pub slug: String,
    pub previous_path: String,
    pub canonical: ForumTopicRouteDescriptor,
    pub alias_id: Option<String>,
    pub changed: bool,
}

pub fn forum_topic_slug_rename_candidate_label(
    candidate: &ForumTopicSlugRenameCandidate,
) -> String {
    format!(
        "{} · {} · /{}",
        candidate.title, candidate.locale, candidate.slug
    )
}

pub fn build_forum_topic_slug_rename_command(
    candidate: &ForumTopicSlugRenameCandidate,
    slug: &str,
) -> Result<ForumTopicSlugRenameCommand, String> {
    let topic_id = candidate.id.trim();
    if !looks_like_uuid(topic_id) {
        return Err("Topic identity is invalid".to_string());
    }

    let locale = candidate.locale.trim();
    if locale.is_empty() {
        return Err("Topic locale is required".to_string());
    }
    if locale.chars().count() > MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN {
        return Err(format!(
            "Topic locale must not exceed {MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN} characters"
        ));
    }
    if locale.chars().any(char::is_control) {
        return Err("Topic locale must not contain control characters".to_string());
    }

    let slug = slug.trim();
    if slug.is_empty() {
        return Err("New topic slug is required".to_string());
    }
    if slug.chars().count() > MAX_FORUM_TOPIC_ROUTE_SLUG_LEN {
        return Err(format!(
            "Topic slug must not exceed {MAX_FORUM_TOPIC_ROUTE_SLUG_LEN} characters"
        ));
    }
    if slug.chars().any(char::is_control) {
        return Err("Topic slug must not contain control characters".to_string());
    }

    Ok(ForumTopicSlugRenameCommand {
        topic_id: topic_id.to_string(),
        locale: locale.to_string(),
        slug: slug.to_string(),
    })
}

fn looks_like_uuid(value: &str) -> bool {
    let groups = value.split('-').collect::<Vec<_>>();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, len)| {
            group.len() == len && group.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> ForumTopicSlugRenameCandidate {
        ForumTopicSlugRenameCandidate {
            id: "00000000-0000-4000-8000-000000000001".to_string(),
            title: "Welcome".to_string(),
            locale: "en".to_string(),
            slug: "welcome".to_string(),
        }
    }

    #[test]
    fn builds_trimmed_owner_command_without_route_policy() {
        let command = build_forum_topic_slug_rename_command(&candidate(), "  New Route  ")
            .expect("rename command");
        assert_eq!(command.topic_id, "00000000-0000-4000-8000-000000000001");
        assert_eq!(command.locale, "en");
        assert_eq!(command.slug, "New Route");
    }

    #[test]
    fn exact_slug_replay_remains_available_to_the_owner() {
        let command = build_forum_topic_slug_rename_command(&candidate(), "welcome")
            .expect("exact replay command");
        assert_eq!(command.slug, "welcome");
    }

    #[test]
    fn rejects_missing_or_unsafe_ui_input() {
        assert!(build_forum_topic_slug_rename_command(&candidate(), "   ").is_err());
        assert!(build_forum_topic_slug_rename_command(&candidate(), "bad\nslug").is_err());
        assert!(
            build_forum_topic_slug_rename_command(
                &candidate(),
                "x".repeat(MAX_FORUM_TOPIC_ROUTE_SLUG_LEN + 1).as_str(),
            )
            .is_err()
        );
    }
}
