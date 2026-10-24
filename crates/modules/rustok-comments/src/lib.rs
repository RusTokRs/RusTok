/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of generic comment threads and non-forum discussion surfaces in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

pub mod dto;

#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod error;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "ports")]
pub mod ports;
#[cfg(feature = "server")]
mod public_read;
#[cfg(feature = "server")]
pub mod remote;
#[cfg(feature = "server")]
mod richtext;
#[cfg(feature = "server")]
pub mod services;
#[cfg(feature = "server")]
mod target_lifecycle;
#[cfg(feature = "tcp-transport")]
pub mod tcp_auth;
#[cfg(feature = "tcp-transport")]
pub mod tcp_channel;
#[cfg(feature = "tcp-transport")]
pub mod tcp_delegation;
#[cfg(feature = "tcp-transport")]
pub mod tcp_delegation_reload;
#[cfg(feature = "tcp-transport")]
pub mod tcp_delegation_schedule;
#[cfg(feature = "tcp-transport")]
mod tcp_protocol;
#[cfg(feature = "tcp-transport")]
pub mod tcp_server;
#[cfg(feature = "tcp-transport")]
pub mod tcp_transport;
#[cfg(feature = "tcp-transport")]
pub mod tcp_transport_reload;


pub use dto::{
    CommentListItem, CommentRecord, CommentStatus, CommentThreadDetail, CommentThreadStatus,
    CommentThreadSummary, CreateCommentInput, GetThreadDetailFilter, ListCommentsFilter,
    ListThreadsFilter, SetCommentStatusRequest, UpdateCommentInput,
};
#[cfg(feature = "server")]
pub use entities::{comment, comment_body, comment_thread, comment_thread_identity_lock};
#[cfg(feature = "server")]
pub use error::{CommentsError, CommentsResult};
#[cfg(feature = "ports")]
pub use ports::CommentsThreadPort;
#[cfg(all(feature = "ports", feature = "server"))]
pub use ports::in_process_comments_thread_port;
#[cfg(feature = "server")]
pub use remote::{
    remote_comments_thread_port, CommentsThreadRequest, CommentsThreadResponse,
    CommentsThreadTransport, CommentsThreadTransportReply, RemoteCommentsThreadPort,
};
#[cfg(feature = "server")]
pub use services::CommentsService;

#[cfg(feature = "tcp-transport")]
pub use tcp_auth::{
    COMMENTS_TCP_PROTOCOL_VERSION, CommentsTcpAuthenticationConfigError,
    CommentsTcpBearerAuthorityResolver, CommentsTcpBearerToken, CommentsTcpCredential,
    CommentsTcpRequestEnvelope,
};
#[cfg(feature = "tcp-transport")]
pub use tcp_channel::{
    BoxCommentsTcpIo, CommentsTcpChannelProtection, CommentsTcpClientChannelConnector,
    CommentsTcpIo, CommentsTcpServerChannelAcceptor, PlaintextLoopbackCommentsTcpChannel,
};
#[cfg(feature = "tcp-transport")]
pub use tcp_delegation::{
    COMMENTS_TCP_DELEGATION_VERSION, CommentsTcpDelegatingAuthorityResolver,
    CommentsTcpDelegationConfigError, CommentsTcpDelegationKeyId, CommentsTcpDelegationKeyring,
    CommentsTcpDelegationSecret, CommentsTcpDelegationSigner,
    DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS, DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
    DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS, MAX_COMMENTS_TCP_DELEGATION_KEY_ID_BYTES,
    MAX_COMMENTS_TCP_DELEGATION_KEYS, MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
    MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
};
#[cfg(feature = "tcp-transport")]
pub use tcp_delegation_reload::{
    CommentsTcpDelegationKeyringProvider, ReloadableCommentsTcpDelegatingAuthorityResolver,
    ReloadableCommentsTcpDelegationSigner,
};
#[cfg(feature = "tcp-transport")]
pub use tcp_delegation_schedule::{
    CommentsTcpDelegationSchedule, CommentsTcpDelegationScheduleConfigError,
    CommentsTcpDelegationScheduledKey, MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS,
    MAX_COMMENTS_TCP_DELEGATION_SCHEDULE_CLOCK_SKEW_MS,
};
#[cfg(feature = "tcp-transport")]
pub use tcp_server::{
    CommentsTcpAuthorityResolver, CommentsTcpOperation, TcpJsonCommentsServerAdapter,
    TrustedCommentsTcpAuthority,
};
#[cfg(feature = "tcp-transport")]
pub use tcp_transport::{CommentsTcpTransportConfigError, TcpJsonCommentsTransport};
#[cfg(feature = "tcp-transport")]
pub use tcp_transport_reload::ReloadableTcpJsonCommentsTransport;

#[cfg(feature = "server")]
pub use module::CommentsModule;

#[cfg(all(test, feature = "server"))]
mod contract_tests;
