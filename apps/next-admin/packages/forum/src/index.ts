import { registerAdminModule } from '@/modules/registry';
import { forumNavItems } from './nav';

registerAdminModule({
  id: 'forum',
  name: 'Forum',
  navItems: forumNavItems
});

export { forumNavItems } from './nav';
export { ForumReplyEditor } from './components/forum-reply-editor';
export * from './api/forum';
