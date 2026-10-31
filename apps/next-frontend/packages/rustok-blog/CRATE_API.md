# @rustok/blog-frontend / CRATE_API

## 1. Public Interfaces & Architecture

`@rustok/blog-frontend` (`apps/next-frontend/packages/rustok-blog`) is the Next.js storefront package for the RusToK blog module. It provides public editorial viewing, taxonomy navigation, search integration, rich text rendering, and threaded comment workflows in full parity with `crates/modules/rustok-blog/storefront` (Leptos).

### Public Exports (`src/index.tsx`)
- **API Functions:**
  - `fetchPublishedPosts(graphql, tenantId, tenantSlug, after, perPage, tag, categoryId, locale): Promise<BlogPostListResponse>`
  - `fetchPublishedPost(graphql, tenantId, tenantSlug, slug, locale, commentsPage, commentsPerPage): Promise<BlogPostDetail | null>`
  - `searchPublishedPosts(graphql, tenantSlug, query, locale, page, pageSize, categoryId, tag): Promise<BlogSearchPage>`
  - `createBlogComment(graphql, tenantId, tenantSlug, token, postId, locale, content, commandId, parentCommentId): Promise<BlogCommentDetail>`
  - `fetchBlogCategories(graphql, tenantId, tenantSlug, locale): Promise<BlogCategorySummary[]>`
  - `fetchBlogTags(graphql, tenantId, tenantSlug, locale): Promise<string[]>`
- **Components:** `BlogSection`, `PostCard`, `BlogCommentComposer`, `ThreadedCommentsList`, `BlogCommentsSection`, `ReactionBar`, `BlogTableOfContents`, `AuthorMiniBadge`, `AuthorBioCard`, `CommentsPagination`, `BlogPagination`, `BlogShareButtons`, `ReadingProgressBar`.
- **Utilities:** `calculateReadingTime`, `formatReadingTime`, `orderThreadedComments`, `buildArticleJsonLd`, `serializeJsonLd`.
- **Slot Registration:** Registers `blog-latest-posts` into slot `home:afterHero` with `order: 20` and `moduleSlug: "blog"`.

---

## 2. Key Contracts, Routes & Layouts

### Data Contracts
- `BlogPostSummary`: Post identity, title, canonical slug, excerpt, featured image URL, author ID, author profile, category, tags, and ISO 8601 publication timestamp.
- `BlogPostDetail`: Full editorial article including `RichTextView` (`content.document` and `content.html`), `contentPlainText`, `seoTitle`, `seoDescription`, version OCC, and `publicComments` availability aggregate.
- `BlogPublicComment`: Public threaded comment node with `effectiveLocale`, `authorId`, `contentPreview`, `parentCommentId`, and `createdAt`.

### Tree Ordering (`orderThreadedComments`)
- Preserves full arbitrary-depth tree hierarchy (`depth` 0, 1, 2...).
- Promotes page-orphaned comments (whose parent is on a different page or missing) to root level so no comments are hidden.
- Breaks cycles and self-parenting loops via tracking sets.

---

## 3. Integration, Transport & Security Boundary

### Transport Boundary
- Dispatches typed GraphQL queries (`PublishedPosts`, `PublishedPost`, `BlogCategories`, `BlogTags`, `BlogFullTextSearch`) and mutation (`CreateBlogComment`).
- Propagates tenant context via `tenantSlug` and `tenantId`.
- Propagates user authentication token via Bearer header on `createBlogComment` with command idempotency (`commandId`).
- Uses shared rich text rendering from `@rustok/richtext` without client-side HTML sanitation bypasses.

---

## 4. Verification & Testing Evidence

- **Unit Tests**:
  - `comments-order.test.mjs`: Verifies arbitrary-depth tree hierarchy, page-orphan reply promotion, and cycle/self-parenting resolution.
  - `reading-time.test.mjs`: Verifies word-count reading estimations and Russian/English pluralization bounds.
- **Storefront Boundary Gate**: `npm run verify:blog:storefront-boundary` confirms strict boundary contracts and rich text sinks.
- **Storefront Route Gate**: `npm run verify:storefront:routes` confirms locale-aware route helpers.
- **Next.js Host Typecheck**: `npm run typecheck:next-frontend` completes with 0 errors.

---

## 5. Anti-Patterns & Operational Invariants

1. **No Post-Filtering Search Keyset**: Full-text queries must go through `searchPublishedPosts` (Search module), not client-side post-filtering on `publicBlogPosts`.
2. **Infinite Pagination Protection**: Taxonomy loaders (`fetchBlogCategories`, `fetchBlogTags`) assert non-empty batches (`batch.items.length === 0`) to prevent infinite loops on broken backend pagination.
3. **No Hidden Comment Rows**: All comments fetched must be rendered; orphaned child comments whose parents are not in the current page are promoted to root rather than discarded.
4. **Exact Locale Propagation**: Reading views forward requested locale and accept backend fallback negotiation (`requested -> tenant default -> first available`).
