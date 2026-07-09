import { registerStorefrontModule } from "@/modules/registry";
import { BlogSection } from "./components/blog-section";

export type { BlogPostSummary, BlogPostListResponse } from "./api/posts";
export { fetchPublishedPosts } from "./api/posts";
export { BlogSection } from "./components/blog-section";
export { PostCard } from "./components/post-card";

registerStorefrontModule({
  id: "blog-latest-posts",
  moduleSlug: "blog",
  slot: "home:afterHero",
  order: 20,
  render: ({ graphql, tenantId, tenantSlug }) => (
    <BlogSection graphql={graphql} tenantId={tenantId} tenantSlug={tenantSlug} />
  ),
});
