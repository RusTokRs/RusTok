use rustok_api::PLATFORM_FALLBACK_LOCALE;
use sea_orm::{
    ColumnTrait, DbBackend, QueryFilter, Select,
    sea_query::{CaseStatement, Expr, ExprTrait, Func, LikeExpr, Query},
};

use crate::dto::PostListQuery;
use crate::entities::{blog_post, blog_post_translation};
use crate::error::{BlogError, BlogResult};

/// Escapes the SQL `LIKE` wildcards (`%`, `_`, `\`) so a text-filter needle
/// matches its characters literally instead of broadening the pattern.
fn escape_like_pattern(needle: &str) -> String {
    let mut escaped = String::with_capacity(needle.len());
    for ch in needle.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Normalizes an optional admin text filter: surrounding whitespace is trimmed,
/// a needle that is empty afterwards means "no filter", and the length is capped
/// so a pathological needle cannot bloat the SQL pattern.
fn normalize_post_text_filter(
    value: Option<&str>,
    field: &'static str,
) -> BlogResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > 255 {
        return Err(BlogError::validation(format!(
            "{field} filter must be at most 255 characters"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// Prepared admin text filters for one list request.
pub(super) struct PostListTextFilters {
    pub slug: Option<String>,
    pub title: Option<String>,
    /// Preferred locales in projection order. The final fallback chooses the
    /// lexicographically smallest canonical stored locale outside this chain.
    pub title_locales: Vec<String>,
}

/// Normalizes the list text filters once and computes the title locale chain,
/// so the query builder receives already-validated needles (or none at all).
pub(super) fn prepare_post_list_text_filters(
    query: &PostListQuery,
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<PostListTextFilters> {
    let title = normalize_post_text_filter(query.title.as_deref(), "title")?;
    let slug = normalize_post_text_filter(query.slug.as_deref(), "slug")?;
    let mut title_locales = Vec::new();
    rustok_api::push_locale_candidate(&mut title_locales, Some(locale), false);
    rustok_api::push_locale_candidate(&mut title_locales, fallback_locale, false);
    rustok_api::push_locale_candidate(&mut title_locales, Some(PLATFORM_FALLBACK_LOCALE), false);
    Ok(PostListTextFilters {
        slug,
        title,
        title_locales,
    })
}

/// Case-insensitive substring filter on the post's own slug column.
pub(super) fn apply_post_slug_filter(
    select: Select<blog_post::Entity>,
    needle: &str,
) -> Select<blog_post::Entity> {
    let pattern = format!("%{}%", escape_like_pattern(needle)).to_lowercase();
    select.filter(
        Func::lower(Expr::col(blog_post::Column::Slug)).like(LikeExpr::new(pattern).escape('\\')),
    )
}

/// Case-insensitive substring filter on the locale-resolved translation title.
///
/// Select the displayed translation before matching, then filter before keyset
/// pagination. Matching every locale could admit a post whose displayed title
/// does not contain the needle and consume a page slot with that false match.
pub(super) fn apply_post_title_filter(
    select: Select<blog_post::Entity>,
    needle: &str,
    locales: &[String],
    backend: DbBackend,
) -> BlogResult<Select<blog_post::Entity>> {
    let pattern = format!("%{}%", escape_like_pattern(needle)).to_lowercase();
    let mut priority = CaseStatement::new();
    for (rank, locale) in locales.iter().enumerate() {
        priority = priority.case(
            blog_post_translation::Column::Locale.eq(locale),
            rank as i32,
        );
    }
    // Owner writes persist canonical locale tags. Their last-resort order must
    // match Rust's bytewise order even on a PostgreSQL linguistic collation.
    let locale = Expr::col(blog_post_translation::Column::Locale);
    let locale_order = match backend {
        DbBackend::Postgres => Expr::cust_with_exprs("$1 COLLATE \"C\"", [locale]),
        DbBackend::Sqlite => Expr::cust_with_exprs("$1 COLLATE BINARY", [locale]),
        _ => {
            return Err(BlogError::invariant(
                "Blog title filtering requires a supported database backend",
            ));
        }
    };
    let displayed_title = Query::select()
        .column(blog_post_translation::Column::Title)
        .from(blog_post_translation::Entity)
        .and_where(
            Expr::col((
                blog_post_translation::Entity,
                blog_post_translation::Column::PostId,
            ))
            .equals((blog_post::Entity, blog_post::Column::Id)),
        )
        .order_by_expr(
            priority.finally(locales.len() as i32).into(),
            sea_orm::Order::Asc,
        )
        .order_by_expr(locale_order, sea_orm::Order::Asc)
        .limit(1)
        .to_owned();

    Ok(select
        .filter(Func::lower(Expr::from(displayed_title)).like(LikeExpr::new(pattern).escape('\\'))))
}
