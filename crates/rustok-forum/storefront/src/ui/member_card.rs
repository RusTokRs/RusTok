use std::{collections::HashMap, sync::Arc};

use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::{i18n::t, model::ForumMemberCard};

pub type ForumMemberCardContext = Arc<HashMap<String, ForumMemberCard>>;

pub fn member_card_context(cards: Vec<ForumMemberCard>) -> ForumMemberCardContext {
    Arc::new(
        cards
            .into_iter()
            .map(|card| (card.user_id.clone(), card))
            .collect(),
    )
}

#[component]
pub fn ForumAuthorBadge(author_id: Option<String>) -> AnyView {
    let card = use_context::<ForumMemberCardContext>().and_then(|cards| {
        author_id
            .as_deref()
            .and_then(|user_id| cards.get(user_id))
            .cloned()
    });

    match card {
        Some(card) => view! { <ForumMemberCardBadge card /> }.into_any(),
        None => ().into_any(),
    }
}

#[component]
fn ForumMemberCardBadge(card: ForumMemberCard) -> AnyView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let topics_label = t(locale.as_deref(), "forum.member.topics", "topics");
    let replies_label = t(locale.as_deref(), "forum.member.replies", "replies");
    let solutions_label = t(locale.as_deref(), "forum.member.solutions", "solutions");
    let initials = profile_initials(&card);
    let display_name = card.profile.display_name.clone();
    let handle = card.profile.handle.clone();
    let avatar_alt = display_name.clone();

    view! {
        <div class="forum-member-card inline-flex max-w-full items-center gap-2 rounded-xl border border-border bg-background/70 px-2.5 py-2">
            <div
                role="img"
                aria-label=avatar_alt
                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-xs font-bold text-primary"
            >
                <span aria-hidden="true">{initials}</span>
            </div>
            <div class="min-w-0">
                <div class="flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-0.5">
                    <span class="truncate text-xs font-semibold text-foreground">{display_name}</span>
                    <span class="truncate text-[11px] text-muted-foreground">{format!("@{handle}")}</span>
                </div>
                <div class="mt-0.5 flex flex-wrap gap-x-2 gap-y-0.5 text-[10px] text-muted-foreground">
                    <span>{format!("{} {topics_label}", card.forum_stats.topic_count)}</span>
                    <span>{format!("{} {replies_label}", card.forum_stats.reply_count)}</span>
                    <span>{format!("{} {solutions_label}", card.forum_stats.solution_count)}</span>
                </div>
            </div>
        </div>
    }
    .into_any()
}

fn profile_initials(card: &ForumMemberCard) -> String {
    let initials = card
        .profile
        .display_name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>();
    if initials.is_empty() {
        card.profile
            .handle
            .chars()
            .next()
            .map(|value| value.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string())
    } else {
        initials.to_uppercase()
    }
}
