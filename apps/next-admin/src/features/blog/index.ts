import { registerAdminModule } from '@/modules/registry';
import { blogNavItems, forumNavItems } from './nav';

registerAdminModule({
  id: 'blog',
  name: 'Blog',
  navItems: blogNavItems
});

registerAdminModule({
  id: 'forum',
  name: 'Forum',
  navItems: forumNavItems
});

// Re-export everything consumers might need
export { blogNavItems, forumNavItems } from './nav';
export { default as PostsPage } from './pages/posts-page';
export { default as PostDetailPage } from './pages/post-detail-page';
export { default as PostFormPage } from './pages/post-form-page';
export { default as PostForm } from './components/post-form';
export { PostTable } from './components/post-table';
export { columns as postColumns } from './components/post-table/columns';
export * from './api/posts';

export { RtJsonEditor } from './components/rt-json-editor';
export { PageBuilder } from './components/page-builder';
export { ForumReplyEditor } from './components/forum-reply-editor';
export * from './api/pages';
export * from './api/forum';
