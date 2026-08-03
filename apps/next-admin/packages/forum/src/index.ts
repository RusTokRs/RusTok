import { registerNavContribution } from '@/shared/lib/app-shell/module-nav-registry';
import { forumNav } from './nav';

export { ForumReplyEditor } from './components/forum-reply-editor';
export { ForumTopicMerge } from './components/forum-topic-merge';
export * from './api/forum';
export * from './core/topic-merge';
export { forumNav } from './nav';

export function registerForumAdmin(): void {
  registerNavContribution({
    id: 'forum.admin',
    moduleSlug: 'forum',
    order: 80,
    item: forumNav
  });
}
