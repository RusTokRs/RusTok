blog-badge = blog
blog-body-rawFormat =
    { $count ->
        [one] Stored in `{ $format }` format. Raw body length: { $count } character.
       *[other] Stored in `{ $format }` format. Raw body length: { $count } characters.
    }
blog-comments-composer-editorLabel = Comment
blog-comments-composer-emptyError = Write a comment before submitting.
blog-comments-composer-hint = Formatting is preserved with the shared richtext editor.
blog-comments-composer-signInRequired = Sign in to join the discussion.
blog-comments-composer-submit = Submit comment
blog-comments-composer-submitting = Submitting...
blog-comments-composer-success = Your comment was submitted for moderation.
blog-comments-composer-title = Join the discussion
blog-comments-createdLabel = created
blog-comments-empty = No approved comments have been published yet.
blog-comments-emptyPage = No approved comments are available on this page.
blog-comments-localeLabel = locale
blog-comments-next = Next
blog-comments-page = Page
blog-comments-previous = Previous
blog-comments-reply = reply
blog-comments-readOnly = Comments are closed for new replies.
blog-comments-title = Comments
blog-comments-total = total
blog-error-load = Failed to load blog storefront data
blog-list-empty = No published blog posts are available for storefront rendering yet.
blog-list-localeLabel = locale
blog-list-noExcerpt = No excerpt yet.
blog-list-open = Open
blog-list-title = Published posts
blog-list-total = total
blog-posts-firstPage = Back to the first page
blog-posts-nextPage = Next page
blog-posts-previousPage = Previous
blog-selected-emptyBody = Open a post from the list below with `?slug=` or publish one from the blog admin package.
blog-selected-emptyTitle = Pick a published post
blog-selected-localeLabel = locale
blog-selected-missingSlug = missing-slug
blog-selected-noBody = No body content yet.
blog-selected-noExcerpt = No excerpt yet.
blog-selected-publishedLabel = published
blog-selected-slugLabel = slug
blog-selected-unscheduled = Unscheduled
blog-subtitle = This storefront surface reads blog data through GraphQL with no host-specific blog wiring.
blog-title = Stories published from the module package
blog-comments-timeout = Comments took too long to load. The article is still available.
blog-comments-timeoutCached = Comments took too long to load. Showing a recent cached snapshot.
blog-comments-unavailable = Comments are temporarily unavailable. The article is still available.
blog-comments-unavailableCached = Comments are temporarily unavailable. Showing a recent cached snapshot.
blog-list-publishedLabel = published
blog-list-unknownStatus = unknown
blog-selected-unknownStatus = unknown
blog-category-all = All
blog-filter-active = Active filters:
blog-filter-clear = Clear all
blog-related-title = Related articles
blog-scrollTop = Scroll to top
blog-search-placeholder = Search articles and tags...

blog-preview-badge = Draft preview
blog-preview-banner-body = This article is not published yet. The link stops working once the post is published, archived or the token expires.
blog-preview-title = Draft preview
blog-preview-subtitle = Paste a preview token, or open the link containing the #token=… fragment.
blog-preview-token-placeholder = token
blog-preview-button-open = Open preview
blog-preview-button-loading = Loading…
blog-preview-error-not-found = The token is invalid, expired or has been revoked.
blog-preview-error-load = Failed to load the draft.

blog-readingTime = { $minutes } min read
blog-share-title = Share
blog-feed-title = RusToK Blog
blog-feed-description = Latest stories and articles from RusToK

blog-comments-cancelReply = Cancel reply
blog-comments-replying = Replying to a comment
blog-comments-replyAction = Reply

blog-search-pages = Search pages
blog-search-results = { $total ->
    [one] Page { $page } · { $total } result
   *[other] Page { $page } · { $total } results
    }

blog-reactions-invalid = Reactions are unavailable for this article.
