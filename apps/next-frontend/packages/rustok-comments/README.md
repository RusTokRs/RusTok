# @rustok/comments-frontend

Comments-owned React storefront authoring UI.

`CommentComposer` owns the canonical richtext draft, validation, busy state, authentication gate, and accessible result feedback. A consuming module supplies an exact owner-bound async submit callback. The component never accepts `target_type`, `target_id`, tenant identity, a GraphQL client, or a locale-selection fallback.

The editor always selects the server-defined `comment` profile and receives
visible copy from the host `Comments.composer` and `richText` catalogs. It does
not reuse the broader Forum `discussion` profile.

Blog is the first consumer. Product reviews and later commentable owners reuse this package and implement only their target-bound backend command.


Write retries are idempotent: the composer creates one command ID for a submitted draft and reuses it when the same draft is retried after an error. Editing the draft invalidates that command ID so a changed payload cannot be replayed under the old request identity.
