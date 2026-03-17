import { registerStorefrontModule } from '@/modules/registry';
import { BlogSection } from './components/blog-section';

registerStorefrontModule({
  id: 'blog-latest-posts',
  moduleSlug: 'blog',
  slot: 'home:afterHero',
  order: 20,
  render: () => <BlogSection />
});

export { BlogSection } from './components/blog-section';
export { PostCard } from './components/post-card';
export { fetchPublishedPosts } from './api/posts';
export type { PublicPostSummary, PublicPostListResponse } from './api/posts';
