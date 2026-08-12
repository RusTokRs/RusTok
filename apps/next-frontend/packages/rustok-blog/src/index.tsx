import { registerStorefrontModule } from "@/modules/registry";
import { BlogSection } from "./components/blog-section";

export type {
  BlogCommentDetail,
  BlogPostDetail,
  BlogPostListResponse,
  BlogPostSummary,
  BlogPublicComment,
} from "./api/posts";
export { createBlogComment, fetchPublishedPost, fetchPublishedPosts } from "./api/posts";
export { BlogSection } from "./components/blog-section";
export { PostCard } from "./components/post-card";

registerStorefrontModule({
  id: "blog-latest-posts",
  moduleSlug: "blog",
  slot: "home:afterHero",
  order: 20,
  render: ({ graphql, tenantId, tenantSlug, locale, searchParams }) => (
    <BlogSection
      graphql={graphql}
      tenantId={tenantId}
      tenantSlug={tenantSlug}
      locale={locale}
      selectedSlug={typeof searchParams.slug === 'string' ? searchParams.slug : null}
    />
  ),
});
