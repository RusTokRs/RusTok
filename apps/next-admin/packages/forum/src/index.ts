import { registerNavContribution } from '@/shared/lib/app-shell/module-nav-registry';
import { forumNav } from './nav';

export { ForumReplyEditor } from './components/forum-reply-editor';
export { ForumTopicFork } from './components/forum-topic-fork';
export { ForumTopicMerge } from './components/forum-topic-merge';
export { ForumTopicReplyRange } from './components/forum-topic-reply-range';
export { ForumTopicSplit } from './components/forum-topic-split';
export * from './api/forum';
export * from './api/topic-reply-range';
export * from './core/topic-fork';
export * from './core/topic-merge';
export * from './core/topic-reply-range';
export * from './core/topic-split';
export { forumNav } from './nav';

export function registerForumAdmin(): void {
  registerNavContribution({
    id: 'forum.admin',
    moduleSlug: 'forum',
    order: 80,
    item: forumNav
  });
}
