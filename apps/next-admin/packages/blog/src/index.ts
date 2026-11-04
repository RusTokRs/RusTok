import { registerAdminModule } from '@/modules/registry';
import { blogNavItems } from './nav';
import { BlogMetricsWidget } from './widgets/blog-dashboard-widgets';

registerAdminModule({
  id: 'blog',
  name: 'Blog',
  navItems: blogNavItems,
  components: [
    {
      id: 'blog:metrics',
      slot: 'dashboard:metric',
      order: 30,
      render: (context) => <BlogMetricsWidget context={context} />
    }
  ]
});

// Re-export everything consumers might need
export { blogNavItems } from './nav';
export { default as PostsPage } from './pages/posts-page';
export { default as PostDetailPage } from './pages/post-detail-page';
export { default as PostFormPage } from './pages/post-form-page';
export { default as PostForm } from './components/post-form';
export { default as CategoriesPage } from './pages/categories-page';
export { CategoryModal } from './components/category-modal';
export { PostTable } from './components/post-table';
export * from './api/posts';
export * from './api/categories';
