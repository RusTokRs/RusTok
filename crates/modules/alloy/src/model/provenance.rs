use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_PROVENANCE_TOOL_NAME_LENGTH: usize = 96;

/// Trusted owner boundary that accepted the source mutation. This is selected
/// by the host; source-bearing requests cannot supply it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoringOrigin {
    Http,
    Graphql,
    RemoteMcp,
    ReleaseImport,
    OwnerRuntime,
}

/// Immutable, content-free provenance attached to each source revision.
///
/// Raw prompts, model completions, MCP arguments, and tool results are never
/// accepted or stored here. An AI-capable owner may attach only a canonical
/// digest of its separately governed prompt record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProvenance {
    pub origin: AuthoringOrigin,
    pub tool_name: Option<String>,
    pub prompt_digest: Option<String>,
}

impl SourceProvenance {
    pub fn http(tool_name: &'static str) -> Self {
        Self::operator(AuthoringOrigin::Http, tool_name)
    }

    pub fn graphql(tool_name: &'static str) -> Self {
        Self::operator(AuthoringOrigin::Graphql, tool_name)
    }

    pub fn remote_mcp(tool_name: &'static str) -> Self {
        Self::operator(AuthoringOrigin::RemoteMcp, tool_name)
    }

    pub fn release_import() -> Self {
        Self {
            origin: AuthoringOrigin::ReleaseImport,
            tool_name: None,
            prompt_digest: None,
        }
    }

    pub fn owner_runtime() -> Self {
        Self {
            origin: AuthoringOrigin::OwnerRuntime,
            tool_name: None,
            prompt_digest: None,
        }
    }

    pub fn with_prompt_digest(mut self, prompt_digest: String) -> Result<Self, ProvenanceError> {
        self.prompt_digest = Some(prompt_digest);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ProvenanceError> {
        let operator_origin = matches!(
            self.origin,
            AuthoringOrigin::Http | AuthoringOrigin::Graphql | AuthoringOrigin::RemoteMcp
        );
        if operator_origin != self.tool_name.is_some()
            || self
                .tool_name
                .as_deref()
                .is_some_and(|tool_name| !valid_tool_name(tool_name))
            || self
                .prompt_digest
                .as_deref()
                .is_some_and(|digest| !canonical_sha256_digest(digest))
        {
            return Err(ProvenanceError::Invalid);
        }
        Ok(())
    }

    fn operator(origin: AuthoringOrigin, tool_name: &'static str) -> Self {
        let provenance = Self {
            origin,
            tool_name: Some(tool_name.to_owned()),
            prompt_digest: None,
        };
        debug_assert!(provenance.validate().is_ok());
        provenance
    }
}

impl Default for SourceProvenance {
    fn default() -> Self {
        Self::owner_runtime()
    }
}

fn valid_tool_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVENANCE_TOOL_NAME_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn canonical_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProvenanceError {
    #[error("Alloy source provenance is invalid")]
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::{AuthoringOrigin, ProvenanceError, SourceProvenance};

    #[test]
    fn provenance_keeps_raw_prompt_content_out_of_the_revision_contract() {
        let provenance = SourceProvenance::remote_mcp("alloy_create_script")
            .with_prompt_digest(format!("sha256:{}", "a".repeat(64)))
            .expect("canonical digest should be accepted");

        assert_eq!(provenance.origin, AuthoringOrigin::RemoteMcp);
        assert_eq!(provenance.tool_name.as_deref(), Some("alloy_create_script"));
        assert!(provenance.prompt_digest.is_some());
        assert_eq!(
            SourceProvenance::remote_mcp("alloy_create_script")
                .with_prompt_digest("raw prompt text".into()),
            Err(ProvenanceError::Invalid)
        );
    }

    #[test]
    fn operator_origins_require_a_canonical_tool_name() {
        let provenance = SourceProvenance {
            origin: AuthoringOrigin::RemoteMcp,
            tool_name: Some("Alloy Create".into()),
            prompt_digest: None,
        };

        assert_eq!(provenance.validate(), Err(ProvenanceError::Invalid));
    }
}
