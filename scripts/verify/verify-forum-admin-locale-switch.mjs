import fs from 'node:fs';

const policyPath = 'crates/rustok-forum/admin/src/locale_switch.rs';
const uiPath = 'crates/rustok-forum/admin/src/ui/leptos.rs';
const categoryDndPath = 'crates/rustok-forum/admin/src/ui/category_dnd.rs';
const transportPath = 'crates/rustok-forum/admin/src/transport/graphql_adapter.rs';
const libPath = 'crates/rustok-forum/admin/src/lib.rs';
const docsPath = 'crates/rustok-forum/docs/forum-28-admin-locale-switch-contract.md';
const enLocalePath = 'crates/rustok-forum/admin/locales/en.json';
const ruLocalePath = 'crates/rustok-forum/admin/locales/ru.json';

for (const path of [
  policyPath,
  uiPath,
  categoryDndPath,
  transportPath,
  libPath,
  docsPath,
  enLocalePath,
  ruLocalePath,
]) {
  if (!fs.existsSync(path)) throw new Error(`missing ${path}`);
}

const policy = fs.readFileSync(policyPath, 'utf8');
const ui = fs.readFileSync(uiPath, 'utf8');
const categoryDnd = fs.readFileSync(categoryDndPath, 'utf8');
const transport = fs.readFileSync(transportPath, 'utf8');
const lib = fs.readFileSync(libPath, 'utf8');
const docs = fs.readFileSync(docsPath, 'utf8');
const enLocale = JSON.parse(fs.readFileSync(enLocalePath, 'utf8'));
const ruLocale = JSON.parse(fs.readFileSync(ruLocalePath, 'utf8'));

const requireAll = (source, markers, label) => {
  for (const marker of markers) {
    if (!source.includes(marker)) throw new Error(`${label}: missing ${marker}`);
  }
};

const enKeys = Object.keys(enLocale).sort();
const ruKeys = Object.keys(ruLocale).sort();
if (JSON.stringify(enKeys) !== JSON.stringify(ruKeys)) {
  const missingInEn = ruKeys.filter((key) => !(key in enLocale));
  const missingInRu = enKeys.filter((key) => !(key in ruLocale));
  throw new Error(
    `forum admin locale bundle key drift: missing in en=[${missingInEn.join(', ')}], missing in ru=[${missingInRu.join(', ')}]`,
  );
}

const ownerCopyKeys = [
  'forum.error.localeSwitchLoad',
  'forum.error.localeSwitchDirty',
  'forum.error.localeSwitchInvalid',
  'forum.error.localeSwitchPending',
  'forum.error.localeSwitchReplyDirty',
  'forum.form.switchLocale',
  'forum.form.localeHintCategory',
  'forum.form.localeHintTopic',
  'forum.error.replyRequired',
  'forum.error.replyTopicRequired',
  'forum.error.saveReply',
  'forum.replies.body',
  'forum.replies.bodyHint',
  'forum.replies.submit',
];

for (const key of ownerCopyKeys) {
  for (const [locale, bundle] of [
    ['en', enLocale],
    ['ru', ruLocale],
  ]) {
    if (typeof bundle[key] !== 'string' || bundle[key].trim() === '') {
      throw new Error(`forum admin owner copy: ${locale} missing non-empty ${key}`);
    }
  }
  if (ruLocale[key] === enLocale[key]) {
    throw new Error(`forum admin owner copy: ru still falls back to English for ${key}`);
  }
}

requireAll(lib, ['mod locale_switch;'], 'lib wiring');

requireAll(policy, [
  'ForumAdminLocaleSwitchDecision',
  'BlockedDirty',
  'category_locale_switch_decision',
  'topic_locale_switch_decision',
  'category_detail_for_editor',
  'topic_detail_for_editor',
  'topic_tags_for_update',
  'category_target_form',
  'topic_target_form',
  'locale_candidate_matches_active',
  'effective_locale',
  'requested_locale',
  'detail.name.clear()',
  'detail.title.clear()',
  'detail.body.document = RichTextDocument::empty()',
  'detail.tags.clear()',
  'candidate_tags == current_tags',
  '&& candidate_tags.is_empty()',
], 'locale policy');

requireAll(transport, [
  'topic_tags_for_update',
  '.map(category_detail_for_editor)',
  '.map(topic_detail_for_editor)',
  'let current = fetch_topic(',
  'let tags = topic_tags_for_update(&current, draft.tags.clone())',
  'update_topic_input(draft, tags)',
  'fn update_topic_input(draft: TopicDraft, tags: Option<Vec<String>>)',
  'tags,',
], 'editor transport fallback/tag guard');

requireAll(ui, [
  'category_locale_input',
  'topic_locale_input',
  'switch_category_locale',
  'switch_topic_locale',
  'category_locale_switch_decision',
  'topic_locale_switch_decision',
  'category_target_form',
  'topic_target_form',
  'locale_candidate_matches_active',
  'ForumAdminLocaleSwitchDecision::BlockedDirty',
  'ForumAdminLocaleSwitchDecision::Reload',
  'transport::fetch_category(',
  'transport::fetch_topic(',
  'ReplyFormSnapshot',
  'content: reply_body.get_untracked()',
  'if is_categories_page',
  'topic_locale.get()',
  'on_locale_switch=switch_category_locale',
  'on_locale_switch=switch_topic_locale',
  'forum.error.localeSwitchLoad',
  'forum.error.localeSwitchDirty',
  'forum.error.localeSwitchInvalid',
  'forum.error.localeSwitchPending',
  'forum.error.localeSwitchReplyDirty',
  'forum_admin_content_lang',
], 'admin UI contract');

requireAll(categoryDnd, [
  'normalize_locale_tag',
  'category_card_content_lang',
  'description_lang',
  'data-forum-target-localized=""',
  'lang=content_lang.clone()',
  'lang=description_lang',
  'dir="auto"',
  'data-forum-route-identifier=""',
  'dir="ltr"',
], 'category card content-locale bidi contract');

if (ui.includes('on:input=move |ev| set_locale.set(event_target_value(&ev))')) {
  throw new Error('locale input must not mutate active owner locale on each keystroke');
}
if (ui.includes('set_locale: WriteSignal<String>')) {
  throw new Error('page locale field must use a candidate signal plus explicit switch callback');
}
if (ui.includes('host_locale_for_seo')) {
  throw new Error('Forum SEO record locale must follow the active content locale, not the host UI locale');
}

const selectedCategoryDisplayInput = ui.indexOf('struct SelectedCategoryDisplay');
const forumAdminInput = ui.indexOf('#[component]\npub fn ForumAdmin()', selectedCategoryDisplayInput);
if (selectedCategoryDisplayInput < 0 || forumAdminInput < 0) {
  throw new Error('missing Forum admin selected category display boundary');
}
const selectedCategoryDisplaySource = ui.slice(selectedCategoryDisplayInput, forumAdminInput);
requireAll(selectedCategoryDisplaySource, [
  'content_lang: Option<String>',
  'fn forum_admin_selected_category_display(',
  'selected_category_filter_label(',
  'Some(Ok(items)) if !selected_id.trim().is_empty()',
  'forum_admin_content_lang(item.effective_locale.as_str())',
  'fn render_selected_category_label(',
  'data-forum-target-localized=""',
  'lang=content_lang',
  'dir="auto"',
  'None => view! { <span>{value.label}</span> }',
], 'selected category display content-locale boundary');

const categoryInput = ui.indexOf('fn CategoriesPage(');
const topicInput = ui.indexOf('fn TopicsPage(');
if (categoryInput < 0 || topicInput < 0) throw new Error('missing forum admin page components');
const categoryPage = ui.slice(categoryInput, topicInput);
const topicPage = ui.slice(topicInput);
for (const [page, source] of [['category', categoryPage], ['topic', topicPage]]) {
  requireAll(source, [
    'locale_input: ReadSignal<String>',
    'set_locale_input: WriteSignal<String>',
    'on_locale_switch: Callback<String>',
    'prop:value=move || locale_input.get()',
    'set_locale_input.set(event_target_value(&ev))',
    'on_locale_switch.run(locale_input.get_untracked())',
    'forum.form.switchLocale',
    'locale=Signal::derive(move || locale.get())',
  ], `${page} locale control`);
}

requireAll(topicPage, [
  'let selected_category_display = Memo::new',
  'forum_admin_selected_category_display(',
  'let sidebar_selected_category = selected_category_display.clone()',
  'let heading_selected_category = selected_category_display',
  'render_selected_category_label(sidebar_selected_category.get())',
  'render_selected_category_label(heading_selected_category.get())',
], 'selected category summary content-locale wiring');
if (topicPage.includes('selected_category_name')) {
  throw new Error('selected category summaries must not collapse content and UI fallback into one plain string');
}

const categorySelectInput = topicPage.indexOf('category_select_options(&items');
const topicTitleInput = topicPage.indexOf(
  '<FieldShell label=topic_form_labels.title_label',
  categorySelectInput,
);
if (categorySelectInput < 0 || topicTitleInput < 0) {
  throw new Error('missing Forum admin category select localized options');
}
const categorySelectSurface = topicPage.slice(categorySelectInput, topicTitleInput);
requireAll(categorySelectSurface, [
  'category_select_options(&items, category_id.get().as_str())',
  '.zip(items.iter())',
  'forum_admin_content_lang(item.effective_locale.as_str())',
  'data-forum-target-localized=""',
  'lang=content_lang',
  'dir="auto"',
], 'category select option content-locale bidi contract');

const tagChipInput = topicPage.indexOf('let parsed_tags = forum_admin_tag_chips');
const richTextInput = topicPage.indexOf('<ForumRichTextEditor', tagChipInput);
if (tagChipInput < 0 || richTextInput < 0) {
  throw new Error('missing Forum admin topic tag chip surface');
}
const tagChipSurface = topicPage.slice(tagChipInput, richTextInput);
requireAll(tagChipSurface, [
  'forum_admin_tag_chips(tags.get().as_str())',
  'data-forum-target-localized=""',
  'lang=move || locale.get()',
  'dir="auto"',
], 'topic tag chip content-locale bidi contract');

const categoryGridInput = ui.indexOf('fn render_category_grid(');
const categorySidebarInput = ui.indexOf('fn render_category_sidebar(');
const topicFeedInput = ui.indexOf('fn render_topic_feed(');
const replyStackInput = ui.indexOf('fn render_reply_stack(');
const applyCategoryInput = ui.indexOf('fn apply_category_to_form(');
if (
  categoryGridInput < 0 ||
  categorySidebarInput < 0 ||
  topicFeedInput < 0 ||
  replyStackInput < 0 ||
  applyCategoryInput < 0
) {
  throw new Error('missing Forum admin localized read surfaces');
}

const categoryGrid = ui.slice(categoryGridInput, categorySidebarInput);
const categorySidebar = ui.slice(categorySidebarInput, topicFeedInput);
const topicFeed = ui.slice(topicFeedInput, replyStackInput);
const replyStack = ui.slice(replyStackInput, applyCategoryInput);

requireAll(categoryGrid, [
  'forum_admin_content_lang(vm.effective_locale.as_str())',
  'let description_lang = if item',
  'forum_admin_content_lang(locale.as_deref().unwrap_or_default())',
  '<div dir="ltr"',
  'data-forum-target-localized=""',
  'lang=content_lang',
  'lang=description_lang',
  'dir="auto"',
  'data-forum-route-identifier=""',
  'dir="ltr"',
], 'fallback category grid content-locale bidi contract');

requireAll(categorySidebar, [
  'forum_admin_content_lang(item.effective_locale.as_str())',
  'data-forum-target-localized=""',
  'lang=content_lang',
  'dir="auto"',
  'data-forum-route-identifier=""',
  'dir="ltr"',
], 'category sidebar content-locale bidi contract');

requireAll(topicFeed, [
  'forum_admin_content_lang(vm.effective_locale.as_str())',
  '<span dir="ltr"',
  'data-forum-target-localized=""',
  'lang=content_lang',
  'dir="auto"',
  'data-forum-route-identifier=""',
  'dir="ltr"',
], 'topic feed content-locale bidi contract');

requireAll(replyStack, [
  'forum_admin_content_lang(vm.effective_locale.as_str())',
  '<span dir="ltr"',
  'data-forum-target-localized=""',
  'lang=content_lang',
  'dir="auto"',
], 'reply preview content-locale bidi contract');

requireAll(docs, [
  'active locale',
  'candidate locale',
  'dirty',
  'fallback',
  'initial editor load',
  'tag labels',
  'preserve',
  'reply',
  'category tree',
  'FORUM-28',
], 'contract docs');

console.log('forum admin locale-switch source contract: OK');
