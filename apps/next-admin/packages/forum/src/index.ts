import { registerAdminModule } from '@/modules/registry';
import { forumNav } from './nav';

registerAdminModule({
  id: 'forum',
  name: 'Forum',
  navItems: [forumNav]
});

export { ForumReplyEditor } from './components/forum-reply-editor';
export { ForumTopicEditor } from './components/forum-topic-editor';
export { ForumTopicFork } from './components/forum-topic-fork';
export { ForumTopicMerge } from './components/forum-topic-merge';
export { ForumTopicReplyRange } from './components/forum-topic-reply-range';
export { ForumTopicSlugRename } from './components/forum-topic-slug-rename';
export { ForumTopicSplit } from './components/forum-topic-split';
export * from './api/forum';
export * from './api/topic-reply-range';
export * from './core/topic-fork';
export * from './core/topic-merge';
export * from './core/topic-reply-range';
export * from './core/topic-slug-rename';
export * from './core/topic-split';
export { forumNav } from './nav';
