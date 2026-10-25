use sea_orm::{Condition, DbBackend, Value, sea_query::Expr};

pub const MAX_SEARCH_QUERY_LEN: usize = 255;

pub fn product_translation_title_search_condition(
    backend: DbBackend,
    _locale: &str,
    search: &str,
) -> Condition {
    let trimmed = search.trim();
    if trimmed.is_empty() {
        return Condition::all();
    }
    let bounded: String = trimmed.chars().take(MAX_SEARCH_QUERY_LEN).collect();
    let pattern = format!("%{bounded}%");

    let exists_sql = match backend {
        DbBackend::Sqlite => {
            "EXISTS (
                SELECT 1
                FROM product_translations pt
                WHERE pt.product_id = products.id
                  AND pt.title LIKE ?
            )"
        }
        _ => {
            "EXISTS (
                SELECT 1
                FROM product_translations pt
                WHERE pt.product_id = products.id
                  AND pt.title LIKE $1
            )"
        }
    };

    Condition::all().add(Expr::cust_with_values(
        exists_sql,
        vec![Value::from(pattern)],
    ))
}

/// Builds a search condition that matches both product translation title (LIKE %search%)
/// and exact locale (`pt.locale = ?` / `pt.locale = $1`).
pub fn product_translation_title_and_locale_search_condition(
    backend: DbBackend,
    locale: &str,
    search: &str,
) -> Condition {
    let trimmed_search = search.trim();
    let trimmed_locale = locale.trim();

    if trimmed_search.is_empty() {
        return Condition::all();
    }

    if trimmed_locale.is_empty() {
        return product_translation_title_search_condition(backend, locale, search);
    }

    let bounded: String = trimmed_search.chars().take(MAX_SEARCH_QUERY_LEN).collect();
    let pattern = format!("%{bounded}%");

    let exists_sql = match backend {
        DbBackend::Sqlite => {
            "EXISTS (
                SELECT 1
                FROM product_translations pt
                WHERE pt.product_id = products.id
                  AND pt.locale = ?
                  AND pt.title LIKE ?
            )"
        }
        _ => {
            "EXISTS (
                SELECT 1
                FROM product_translations pt
                WHERE pt.product_id = products.id
                  AND pt.locale = $1
                  AND pt.title LIKE $2
            )"
        }
    };

    Condition::all().add(Expr::cust_with_values(
        exists_sql,
        vec![Value::from(trimmed_locale.to_string()), Value::from(pattern)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_condition_empty_or_whitespace_returns_empty_condition() {
        let cond_empty = product_translation_title_search_condition(DbBackend::Postgres, "en", "");
        let cond_whitespace =
            product_translation_title_search_condition(DbBackend::Sqlite, "en", "   ");

        assert!(cond_empty.is_empty());
        assert!(cond_whitespace.is_empty());
    }

    #[test]
    fn test_search_condition_trimmed_query() {
        let cond = product_translation_title_search_condition(DbBackend::Postgres, "en", "  shoe  ");
        assert!(!cond.is_empty());
    }

    #[test]
    fn test_localized_search_condition() {
        let cond = product_translation_title_and_locale_search_condition(
            DbBackend::Postgres,
            "ru",
            "кроссовки",
        );
        assert!(!cond.is_empty());

        let cond_empty_search =
            product_translation_title_and_locale_search_condition(DbBackend::Postgres, "ru", "   ");
        assert!(cond_empty_search.is_empty());

        let cond_empty_locale_falls_back = product_translation_title_and_locale_search_condition(
            DbBackend::Postgres,
            "   ",
            "кроссовки",
        );
        assert!(!cond_empty_locale_falls_back.is_empty());
    }

    #[test]
    fn test_excessive_search_query_truncated() {
        let long_query = "a".repeat(500);
        let cond = product_translation_title_search_condition(DbBackend::Sqlite, "en", &long_query);
        assert!(!cond.is_empty());
    }
}
