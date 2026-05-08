use sea_orm::{sea_query::Expr, Condition, DbBackend, Value};

pub fn product_translation_title_search_condition(
    backend: DbBackend,
    _locale: &str,
    search: &str,
) -> Condition {
    let pattern = format!("%{search}%");

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
