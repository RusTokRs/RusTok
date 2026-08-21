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
  ], `${page} locale control`);
}

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
