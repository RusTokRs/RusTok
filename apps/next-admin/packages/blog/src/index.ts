import { registerAdminModule } from '@/modules/registry';
import { blogNavItems } from './nav';

registerAdminModule({
  id: 'blog',
  name: 'Blog',
  navItems: blogNavItems
});

// Re-export everything consumers might need
export { blogNavItems } from './nav';
export { default as PostsPage } from './pages/posts-page';
export { default as PostDetailPage } from './pages/post-detail-page';
export { default as PostFormPage } from './pages/post-form-page';
export { default as PostForm } from './components/post-form';
export { PostTable } from './components/post-table';
export { columns as postColumns } from './components/post-table/columns';
export * from './api/posts';
