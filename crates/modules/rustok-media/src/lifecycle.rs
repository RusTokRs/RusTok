use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetState {
    Active,
    DeletePending,
    Deleted,
    Failed,
}

impl AssetState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::DeletePending => "delete_pending",
            Self::Deleted => "deleted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlobState {
    Pending,
    Ready,
    DeletePending,
    Deleted,
    Failed,
}

impl BlobState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::DeletePending => "delete_pending",
            Self::Deleted => "deleted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenditionState {
    Pending,
    Processing,
    Ready,
    Failed,
}

impl RenditionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadState {
    Pending,
    Finalizing,
    Completed,
    Expired,
    Failed,
}

impl UploadState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Finalizing => "finalizing",
            Self::Completed => "completed",
            Self::Expired => "expired",
            Self::Failed => "failed",
        }
    }
}
