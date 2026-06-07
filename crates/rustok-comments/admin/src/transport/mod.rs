//! Module-owned transport facade for the comments admin UI.
//!
//! The current admin surface intentionally exposes a temporary single-adapter
//! native server-function transport. Keeping this facade between `ui/leptos` and
//! adapter implementation code prevents render code from owning transport calls.
//! `rustok-comments` has no legacy GraphQL/REST admin surface, so this package
//! keeps the native-only exception explicit instead of inventing a local
//! headless fallback contract.

pub(crate) mod native_server_adapter;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::{
    CommentThreadDetailRequest, CommentThreadsRequest, SetCommentStatusCommand,
    SetThreadStatusCommand,
};
use rustok_comments::{CommentRecord, CommentThreadDetail, CommentThreadSummary};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum CommentsAdminTransportError {
    ServerFn(String),
}

impl Display for CommentsAdminTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CommentsAdminTransportError {}

impl From<ServerFnError> for CommentsAdminTransportError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CommentThreadsPayload {
    pub items: Vec<CommentThreadSummary>,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommentsAdminTransportPath {
    NativeServer,
}

pub(crate) const ACTIVE_TRANSPORT_PATH: CommentsAdminTransportPath =
    CommentsAdminTransportPath::NativeServer;

pub(crate) async fn fetch_threads(
    request: CommentThreadsRequest,
) -> Result<CommentThreadsPayload, CommentsAdminTransportError> {
    match ACTIVE_TRANSPORT_PATH {
        CommentsAdminTransportPath::NativeServer => native_server_adapter::fetch_threads(request)
            .await
            .map_err(Into::into),
    }
}

pub(crate) async fn fetch_thread_detail(
    request: CommentThreadDetailRequest,
) -> Result<CommentThreadDetail, CommentsAdminTransportError> {
    match ACTIVE_TRANSPORT_PATH {
        CommentsAdminTransportPath::NativeServer => {
            native_server_adapter::fetch_thread_detail(request)
                .await
                .map_err(Into::into)
        }
    }
}

pub(crate) async fn set_thread_status(
    command: SetThreadStatusCommand,
) -> Result<CommentThreadSummary, CommentsAdminTransportError> {
    match ACTIVE_TRANSPORT_PATH {
        CommentsAdminTransportPath::NativeServer => {
            native_server_adapter::set_thread_status(command)
                .await
                .map_err(Into::into)
        }
    }
}

pub(crate) async fn set_comment_status(
    command: SetCommentStatusCommand,
) -> Result<CommentRecord, CommentsAdminTransportError> {
    match ACTIVE_TRANSPORT_PATH {
        CommentsAdminTransportPath::NativeServer => {
            native_server_adapter::set_comment_status(command)
                .await
                .map_err(Into::into)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_transport_path_documents_temporary_native_only_state() {
        assert_eq!(
            ACTIVE_TRANSPORT_PATH,
            CommentsAdminTransportPath::NativeServer
        );
    }
}
