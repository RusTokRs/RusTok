use sea_orm::{Condition, DbBackend, Value, sea_query::Expr};

pub fn product_translation_title_search_condition(
    backend: DbBackend,
    _locale: &str,
    search: &str,
) -> Condition {
    let trimmed = search.trim();
    if trimmed.is_empty() {
        return Condition::all();
    }
    let pattern = format!("%{trimmed}%");

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
}
