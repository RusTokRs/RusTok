use leptos::prelude::*;

use crate::core::{
    CommentThreadDetailRequest, CommentThreadsRequest, SetCommentStatusCommand,
    SetThreadStatusCommand,
};
use rustok_comments::{
    CommentRecord, CommentStatus, CommentThreadDetail, CommentThreadStatus, CommentThreadSummary,
};

use super::CommentThreadsPayload;

pub(crate) async fn fetch_threads(
    request: CommentThreadsRequest,
) -> Result<CommentThreadsPayload, ServerFnError> {
    comments_threads_native(
        request.page,
        request.per_page,
        request.target_type,
        request.thread_status,
        request.comment_status,
    )
    .await
}

pub(crate) async fn fetch_thread_detail(
    request: CommentThreadDetailRequest,
) -> Result<CommentThreadDetail, ServerFnError> {
    comments_thread_detail_native(
        request.thread_id,
        request.locale,
        request.page,
        request.per_page,
    )
    .await
}

pub(crate) async fn set_thread_status(
    command: SetThreadStatusCommand,
) -> Result<CommentThreadSummary, ServerFnError> {
    comments_set_thread_status_native(command.thread_id, command.status).await
}

pub(crate) async fn set_comment_status(
    command: SetCommentStatusCommand,
) -> Result<CommentRecord, ServerFnError> {
    comments_set_comment_status_native(command.comment_id, command.status, command.locale).await
}

#[server(prefix = "/api/fn", endpoint = "comments/threads")]
async fn comments_threads_native(
    page: u64,
    per_page: u64,
    target_type: String,
    thread_status: Option<CommentThreadStatus>,
    comment_status: Option<CommentStatus>,
) -> Result<CommentThreadsPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let service = rustok_comments::CommentsService::new(app_ctx.db.clone());
        let security = auth.security_context();
        let (items, total) = service
            .list_threads(
                tenant.id,
                security,
                page.max(1),
                per_page.max(1),
                Some(target_type.as_str()).filter(|value| !value.trim().is_empty()),
                thread_status,
                comment_status,
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(CommentThreadsPayload { items, total })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (page, per_page, target_type, thread_status, comment_status);
        Err(ServerFnError::new(
            "comments/threads requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "comments/thread-detail")]
async fn comments_thread_detail_native(
    thread_id: String,
    locale: String,
    page: u64,
    per_page: u64,
) -> Result<CommentThreadDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let service = rustok_comments::CommentsService::new(app_ctx.db.clone());
        let security = auth.security_context();
        let thread_id = uuid::Uuid::parse_str(&thread_id).map_err(ServerFnError::new)?;
        service
            .get_thread_detail(
                tenant.id,
                security,
                thread_id,
                &locale,
                Some(tenant.default_locale.as_str()),
                page.max(1),
                per_page.max(1),
            )
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (thread_id, locale, page, per_page);
        Err(ServerFnError::new(
            "comments/thread-detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "comments/set-thread-status")]
async fn comments_set_thread_status_native(
    thread_id: String,
    status: CommentThreadStatus,
) -> Result<CommentThreadSummary, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let service = rustok_comments::CommentsService::new(app_ctx.db.clone());
        let security = auth.security_context();
        let thread_id = uuid::Uuid::parse_str(&thread_id).map_err(ServerFnError::new)?;
        service
            .set_thread_status(tenant.id, security, thread_id, status)
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (thread_id, status);
        Err(ServerFnError::new(
            "comments/set-thread-status requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "comments/set-comment-status")]
async fn comments_set_comment_status_native(
    comment_id: String,
    status: CommentStatus,
    locale: String,
) -> Result<CommentRecord, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let service = rustok_comments::CommentsService::new(app_ctx.db.clone());
        let security = auth.security_context();
        let comment_id = uuid::Uuid::parse_str(&comment_id).map_err(ServerFnError::new)?;
        service
            .set_comment_status(
                tenant.id,
                security,
                comment_id,
                status,
                &locale,
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (comment_id, status, locale);
        Err(ServerFnError::new(
            "comments/set-comment-status requires the `ssr` feature",
        ))
    }
}
