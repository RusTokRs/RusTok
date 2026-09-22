/// Distributed Tracing Utilities
///
/// Helper functions and macros for creating standardized spans across RusToK.
///
/// # Features
/// - Standardized span creation with common attributes
/// - Tenant and user correlation
/// - Error span recording
/// - Performance tracking
use tracing::Span;
use uuid::Uuid;

/// Common span attributes for RusToK operations
pub struct SpanAttributes {
    pub tenant_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub operation: String,
    pub module: String,
}

impl SpanAttributes {
    pub fn new(operation: impl Into<String>, module: impl Into<String>) -> Self {
        Self {
            tenant_id: None,
            user_id: None,
            operation: operation.into(),
            module: module.into(),
        }
    }

    pub fn with_tenant(mut self, tenant_id: Uuid) -> Self {
        self.tenant_id = Some(tenant_id);
        self
    }

    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }
}

/// Create a standardized info span
///
/// # Example
/// ```no_run
/// use rustok_core::tracing::{create_span, SpanAttributes};
/// use uuid::Uuid;
///
/// let tenant_id = Uuid::new_v4();
/// let attrs = SpanAttributes::new("fetch_user", "users")
///     .with_tenant(tenant_id);
///
/// let span = create_span("fetch_user", attrs);
/// let _guard = span.enter();
/// // Your code here
/// ```
pub fn create_span(span_name: &str, attrs: SpanAttributes) -> Span {
    let span = tracing::info_span!(
        "rustok_operation",
        span.name = span_name,
        module = %attrs.module,
        operation = %attrs.operation,
        tenant_id = tracing::field::Empty,
        user_id = tracing::field::Empty,
        error = tracing::field::Empty,
        error_type = tracing::field::Empty,
        error_occurred = tracing::field::Empty,
        success = tracing::field::Empty,
        result = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
    );

    if let Some(tenant_id) = attrs.tenant_id {
        span.record("tenant_id", tracing::field::display(tenant_id));
    }

    if let Some(user_id) = attrs.user_id {
        span.record("user_id", tracing::field::display(user_id));
    }

    span
}

/// Record an error in the current span
///
/// # Example
/// ```no_run
/// use rustok_core::tracing::record_error;
///
/// let result: Result<(), &str> = Err("something failed");
/// if let Err(e) = result {
///     record_error(e, "operation_failed");
/// }
/// ```
pub fn record_error<E: std::fmt::Display>(error: E, error_type: &str) {
    let span = Span::current();
    span.record("error", tracing::field::display(&error));
    span.record("error_type", error_type);
    span.record("error_occurred", true);
}

/// Record operation success with optional result data
pub fn record_success(message: &str) {
    let span = Span::current();
    span.record("success", true);
    span.record("result", message);
}

/// Create a database query span
///
/// # Example
/// ```no_run
/// use rustok_core::tracing::db_span;
///
/// let span = db_span("SELECT * FROM users WHERE id = $1", "postgres");
/// let _guard = span.enter();
/// // Execute query
/// ```
pub fn db_span(query: &str, db_type: &str) -> Span {
    tracing::debug_span!(
        "db.query",
        db.type = db_type,
        db.operation = query,
        otel.kind = "client",
    )
}

/// Create an HTTP client span
pub fn http_client_span(method: &str, url: &str) -> Span {
    tracing::info_span!(
        "http.client",
        http.method = method,
        http.url = url,
        otel.kind = "client",
    )
}

/// Create an event processing span
pub fn event_span(event_type: &str, event_id: &str) -> Span {
    tracing::info_span!(
        "event.process",
        event.type = event_type,
        event.id = event_id,
        otel.kind = "consumer",
    )
}

/// Macro for executing a block within a standardized tracing span.
///
/// # Example
/// ```ignore
/// use rustok_core::traced;
/// use uuid::Uuid;
///
/// let tenant_id = Uuid::new_v4();
/// let user_id = Uuid::new_v4();
/// let value = traced!(name = "fetch_user", tenant_id, user_id => {
///     42
/// });
/// assert_eq!(value, 42);
/// ```
#[macro_export]
macro_rules! traced {
    (
        name = $name:expr,
        $($field:ident),* => $body:block
    ) => {
        {
            let __span = tracing::info_span!(
                $name,
                $($field = tracing::field::Empty,)*
            );
            $(
                __span.record(
                    stringify!($field),
                    tracing::field::display(&$field)
                );
            )*
            let __guard = __span.enter();
            let __result = { $body };
            drop(__guard);
            __result
        }
    };
}

/// Measure operation duration and record as span attribute
///
/// # Example
/// ```no_run
/// use rustok_core::tracing::measure;
///
/// # async fn example() {
/// let result = measure("slow_operation", || async {
///     // Your slow operation
///     tokio::time::sleep(std::time::Duration::from_millis(100)).await;
///     42
/// }).await;
/// # }
/// ```
pub async fn measure<F, Fut, T>(operation: &str, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = std::time::Instant::now();
    let result = f().await;
    let duration = start.elapsed();

    let span = Span::current();
    span.record("duration_ms", duration.as_millis() as u64);
    span.record("operation", operation);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_attributes() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let attrs = SpanAttributes::new("test_op", "test_module")
            .with_tenant(tenant_id)
            .with_user(user_id);

        assert_eq!(attrs.operation, "test_op");
        assert_eq!(attrs.module, "test_module");
        assert_eq!(attrs.tenant_id, Some(tenant_id));
        assert_eq!(attrs.user_id, Some(user_id));
    }

    #[test]
    fn test_create_span() {
        let attrs = SpanAttributes::new("test", "module");
        let span = create_span("test_span", attrs);
        assert_eq!(span.metadata().unwrap().name(), "rustok_operation");
    }

    #[tokio::test]
    async fn test_measure() {
        let result = measure("test", || async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42
        })
        .await;

        assert_eq!(result, 42);
    }

    #[test]
    fn test_traced_macro_returns_block_result() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let result = traced!(name = "test.traced", tenant_id, user_id => {
            format!("{tenant_id}:{user_id}")
        });

        assert!(result.contains(':'));
    }
}
