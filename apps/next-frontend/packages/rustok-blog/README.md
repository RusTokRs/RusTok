# @rustok/blog-frontend

Next.js storefront package for RusToK Blog articles, rich editorial reading experiences, threaded comments, author cards, reactions, and taxonomy discovery.

## Features

- **Storefront Slot Registration:** Automatically registers the `blog-latest-posts` widget into the `home:afterHero` slot via `@/modules/registry`.
- **Editorial Post View:** Renders rich text articles via `@rustok/richtext` with reading time estimation (`calculateReadingTime`), table of contents (`BlogTableOfContents`), and reading progress bars (`ReadingProgressBar`).
- **Threaded Commenting:** Full hierarchical comment rendering (`ThreadedCommentsList`) with deep tree traversal, cycle resilience, orphan promotion, and rich text comment composition (`BlogCommentComposer`).
- **Taxonomy & Search:** Full-text post searching (`searchPublishedPosts`) and complete dictionary pagination for categories (`fetchBlogCategories`) and tags (`fetchBlogTags`).
- **SEO & Social Sharing:** Native JSON-LD article structured data generation (`buildArticleJsonLd`) and share button toolbars (`BlogShareButtons`).
