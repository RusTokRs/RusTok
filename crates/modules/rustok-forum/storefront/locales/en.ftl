forum-badge = forum
forum-categories-label = Categories
forum-categories-noDescription = No description yet.
forum-categories-title = Community map
forum-categories-total =
    { $count ->
        [one] { $count } section published from the forum module.
       *[other] { $count } sections published from the forum module.
    }
forum-composer-bodyPlaceholder = Write your post here. A blank line starts a new paragraph.
forum-composer-cancel = Cancel
forum-composer-categoryPlaceholder = Select a category
forum-composer-close = Close
forum-composer-draft = Draft
forum-composer-draftActive = Draft active
forum-composer-expand = Expand
forum-composer-minimize = Minimize
forum-composer-newTopicTitle = Create new topic
forum-composer-previewEmpty = Nothing to preview yet.
forum-composer-previewHint = Live preview
forum-composer-quotesLabel = Quotes
forum-composer-replyTitle = Reply
forum-composer-submitFailed = The forum rejected this post
forum-composer-submitReply = Post reply
forum-composer-submitTopic = Create topic
forum-composer-submitting = Posting...
forum-composer-tabPreview = Preview
forum-composer-tabWrite = Write
forum-composer-topicTitlePlaceholder = Topic title...
forum-error-loadStorefront = Failed to load forum storefront data
forum-error-updateReadState = Failed to update forum read state
forum-feed-emptyBody = Publish a topic from the forum admin package to light up this storefront feed.
forum-feed-emptyTitle = No topics yet
forum-feed-label = Topic feed
forum-feed-threads =
    { $count ->
        [one] { $count } thread
       *[other] { $count } threads
    }
forum-feed-title = Latest discussions
forum-member-replies = replies
forum-member-solutions = solutions
forum-member-topics = topics
forum-richContent-summary =
    { $count ->
        [one] Stored in `{ $format }` format. Raw content length: { $count } character.
       *[other] Stored in `{ $format }` format. Raw content length: { $count } characters.
    }
forum-subtitle = A NodeBB-inspired storefront surface that reads categories, topic feed, and thread replies through the forum module's public GraphQL contract.
forum-thread-markRead = Mark topic read
forum-thread-markingRead = Marking read…
forum-thread-noReplies = No replies yet.
forum-thread-openBody = Pick a topic from the feed to read the opening post and latest replies.
forum-thread-openTitle = Open a thread
forum-thread-quote = Quote
forum-thread-quoteTitle = Quote this reply
forum-thread-repliesTitle = Replies
# plural-exempt: adjectival 'N total' is invariant in English
forum-thread-repliesTotal = { $count } total
forum-thread-reply = Reply
forum-thread-replyClosed = Replies are closed for this topic.
forum-thread-slug = slug: { $slug }
forum-title = Community threads from the module package
forum-topic-locked = Locked
forum-topic-pinned = Pinned
forum-topic-replies = Replies
forum-topic-slug = thread slug: { $slug }
# plural-exempt: adjectival 'N unread' is invariant in English
forum-topic-unreadCount = { $count } unread
forum-topic-updatedUnread = Updated
forum-view-categories = Categories
forum-view-discussions = Discussions
