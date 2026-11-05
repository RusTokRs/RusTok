use sea_orm::{ColumnTrait, Condition, Order, QueryOrder, Select, sea_query::NullOrdering};

use crate::dto::PublishedPostCursor;
use crate::entities::blog_post;
use crate::error::{BlogError, BlogResult};

/// The one ordering of every public Blog post list:
/// `is_pinned DESC, pinned_at DESC NULLS LAST, published_at DESC, id DESC`.
///
/// It is expressed as a column order with explicit NULL placement, not as an
/// `IS NULL` expression, so `idx_blog_posts_public_keyset` can serve it. The
/// public keyset cursor resumes through [`public_post_after_condition`], which
/// expands the same keys lexicographically.
pub(crate) fn order_public_post_list(
    select: Select<blog_post::Entity>,
) -> Select<blog_post::Entity> {
    select
        .order_by_desc(blog_post::Column::IsPinned)
        .order_by_with_nulls(blog_post::Column::PinnedAt, Order::Desc, NullOrdering::Last)
        .order_by_desc(blog_post::Column::PublishedAt)
        .order_by_desc(blog_post::Column::Id)
}

/// Cursor that resumes the public list directly after `post` in
/// [`order_public_post_list`].
pub(crate) fn public_post_cursor(post: &blog_post::Model) -> BlogResult<PublishedPostCursor> {
    let published_at = post.published_at.ok_or_else(|| {
        BlogError::invariant(format!(
            "Published blog post {} has no published_at",
            post.id
        ))
    })?;
    Ok(PublishedPostCursor {
        is_pinned: post.is_pinned,
        pinned_at: post.pinned_at,
        published_at,
        id: post.id,
    })
}

/// Rows strictly after `after` in [`order_public_post_list`], expanded
/// lexicographically over the four sort keys.
///
/// The pinned rank is boolean, so it is compared as `is_pinned DESC`: a cursor
/// on a pinned row also matches every unpinned row, and a cursor on an unpinned
/// row matches none. Within the same rank, `pinned_at` sorts `NULLS LAST`, so a
/// non-NULL cursor value is followed by smaller values and every NULL.
pub(crate) fn public_post_after_condition(after: &PublishedPostCursor) -> Condition {
    let pinned_eq = blog_post::Column::IsPinned.eq(after.is_pinned);
    let pinned_at_eq = match after.pinned_at {
        Some(value) => blog_post::Column::PinnedAt.eq(value),
        None => blog_post::Column::PinnedAt.is_null(),
    };

    let mut condition = Condition::any().add(blog_post::Column::IsPinned.lt(after.is_pinned));
    if let Some(pinned_at) = after.pinned_at {
        condition = condition.add(
            Condition::all().add(pinned_eq.clone()).add(
                Condition::any()
                    .add(blog_post::Column::PinnedAt.lt(pinned_at))
                    .add(blog_post::Column::PinnedAt.is_null()),
            ),
        );
    }
    // Within the same pinned rank and the same pin timestamp, earlier publication
    // order wins. A NULL cursor timestamp only matches the NULL group.
    condition = condition.add(
        Condition::all()
            .add(pinned_eq.clone())
            .add(pinned_at_eq.clone())
            .add(blog_post::Column::PublishedAt.lt(after.published_at)),
    );
    condition.add(
        Condition::all()
            .add(pinned_eq)
            .add(pinned_at_eq)
            .add(blog_post::Column::PublishedAt.eq(after.published_at))
            .add(blog_post::Column::Id.lt(after.id)),
    )
}

/// Ordering of the published corpus for **projections** (Search ingestion,
/// bulk scans): `published_at DESC, id DESC`.
///
/// Deliberately not [`order_public_post_list`]: that ordering leads with
/// `is_pinned`/`pinned_at`, which are *mutable presentation state* — `pin_post`
/// and `unpin_post` rewrite them without emitting a reindex request (the Search
/// payload does not project pin state), so a row can move across a cursor while
/// a page-by-page walk is in flight. For a reader that is one request that is
/// harmless; for an indexer walk that deletes the tenant's documents first and
/// re-inserts them page by page, a post pinned (or created and pinned) between
/// two pages jumps *before* the cursor and is never visited in that pass — and
/// no later event re-indexes it. Ordering by the two immutable publication keys
/// keeps every walk complete; the tie-break on `id` makes it total.
pub(crate) fn order_published_post_scan(
    select: Select<blog_post::Entity>,
) -> Select<blog_post::Entity> {
    select
        .order_by_desc(blog_post::Column::PublishedAt)
        .order_by_desc(blog_post::Column::Id)
}

/// Cursor that resumes [`order_published_post_scan`] after `post`.
///
/// The pin keys of [`PublishedPostCursor`] are written as their neutral values:
/// they are not part of this ordering, and the shape is shared so that an
/// already-issued token keeps parsing. They are ignored on the way back in by
/// [`published_post_scan_after_condition`].
pub(crate) fn published_post_scan_cursor(
    post: &blog_post::Model,
) -> BlogResult<PublishedPostCursor> {
    let published_at = post.published_at.ok_or_else(|| {
        BlogError::invariant(format!(
            "Published blog post {} has no published_at",
            post.id
        ))
    })?;
    Ok(PublishedPostCursor {
        is_pinned: false,
        pinned_at: None,
        published_at,
        id: post.id,
    })
}

/// Rows strictly after `after` in [`order_published_post_scan`].
///
/// Only the two immutable keys are compared. Reusing
/// [`public_post_after_condition`] here would silently restrict the walk to
/// `is_pinned = false AND pinned_at IS NULL`, so every pinned post would be
/// skipped.
pub(crate) fn published_post_scan_after_condition(after: &PublishedPostCursor) -> Condition {
    Condition::any()
        .add(blog_post::Column::PublishedAt.lt(after.published_at))
        .add(
            Condition::all()
                .add(blog_post::Column::PublishedAt.eq(after.published_at))
                .add(blog_post::Column::Id.lt(after.id)),
        )
}
