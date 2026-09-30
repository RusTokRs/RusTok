comments-badge = comments
comments-comment-approve = Approve
comments-comment-pending = Pending
comments-comment-spam = Spam
comments-comment-trash = Trash
comments-detail-authorLine = author { $author } · { $created_at }
comments-detail-localeLine = locale { $requested } -> { $effective }
comments-detail-statusLine =
    { $count ->
        [one] { $count } comment, status { $status }
       *[other] { $count } comments, status { $status }
    }
comments-detail-thread = Thread
comments-detail-title = Thread Detail
comments-error-loadThreads = Failed to load threads
comments-error-selectThread = Select a thread first
comments-error-updateComment = Failed to update comment
comments-error-updateThread = Failed to update thread
comments-filters-allCommentStatuses = All comment statuses
comments-filters-allThreadStatuses = All thread statuses
comments-filters-localePlaceholder = Locale
comments-filters-targetTypePlaceholder = Target type
comments-pagination-next = Next
comments-pagination-page = Page { $page }
comments-pagination-prev = Prev
comments-subtitle = Module-owned moderation surface for generic non-forum comments. This UI is native-first and intentionally does not invent a new GraphQL or REST transport.
comments-thread-closed = Close
comments-thread-open = Open
comments-threads-count =
    { $count ->
        [one] { $count } comment
       *[other] { $count } comments
    }
comments-threads-title = Threads
comments-threads-total =
    { $count ->
        [one] { $count } matching thread
       *[other] { $count } matching threads
    }
comments-title = Comments Moderation
