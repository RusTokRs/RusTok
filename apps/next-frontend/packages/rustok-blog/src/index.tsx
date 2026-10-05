import { registerStorefrontModule } from "@/modules/registry";
import { BlogSection } from "./components/blog-section";

export type {
  BlogCommentDetail,
  BlogPostAuthorProfile,
  BlogPostDetail,
  BlogPostListResponse,
  BlogPostSummary,
  BlogPublicComment,
} from "./api/posts";
export { createBlogComment, fetchPublishedPost, fetchPublishedPosts } from "./api/posts";
export { BlogSection } from "./components/blog-section";
export { PostCard } from "./components/post-card";
export { BlogCommentComposer } from "./components/blog-comment-composer";
export { BlogTableOfContents } from "./components/blog-toc";
export { AuthorMiniBadge, AuthorBioCard } from "./components/author-card";
export { CommentsPagination } from "./components/comments-pagination";
export { BlogPagination } from "./components/blog-pagination";
export { BlogShareButtons } from "./components/blog-share-buttons";
export { calculateReadingTime, formatReadingTime } from "./utils/reading-time";
export { buildArticleJsonLd } from "./utils/seo-json-ld";

registerStorefrontModule({
  id: "blog-latest-posts",
  moduleSlug: "blog",
  slot: "home:afterHero",
  order: 20,
  render: ({ tenantId, tenantSlug, locale, searchParams }) => (
    <BlogSection
      tenantId={tenantId}
      tenantSlug={tenantSlug}
      locale={locale}
      selectedSlug={typeof searchParams.slug === 'string' ? searchParams.slug : null}
    />
  ),
});
