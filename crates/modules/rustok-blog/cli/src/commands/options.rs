use rustok_cli_core::{CliCoreError, CliCoreResult};
use uuid::Uuid;

pub(crate) fn parse_optional_tenant_id(
    options: &serde_json::Map<String, serde_json::Value>,
) -> CliCoreResult<Option<Uuid>> {
    let value = options
        .get("tenant_id")
        .or_else(|| options.get("tenant-id"));
    let Some(val) = value else {
        return Ok(None);
    };

    let str_val = val.as_str().ok_or_else(|| CliCoreError::InvalidInput {
        message: "--tenant-id must be a UUID string".to_string(),
    })?;

    let trimmed = str_val.trim();
    if trimmed.is_empty() {
        return Err(CliCoreError::InvalidInput {
            message: "--tenant-id cannot be empty".to_string(),
        });
    }

    let parsed = Uuid::parse_str(trimmed).map_err(|_| CliCoreError::InvalidInput {
        message: format!("--tenant-id '{trimmed}' must be a valid UUID"),
    })?;

    if parsed.is_nil() {
        return Err(CliCoreError::InvalidInput {
            message: "--tenant-id cannot be nil UUID".to_string(),
        });
    }

    Ok(Some(parsed))
}

pub(crate) fn parse_limit(
    options: &serde_json::Map<String, serde_json::Value>,
    default_limit: u32,
    max_limit: u32,
) -> CliCoreResult<u32> {
    let Some(value) = options.get("limit") else {
        return Ok(default_limit);
    };

    let limit = match value {
        serde_json::Value::Number(n) => n.as_u64().ok_or_else(|| CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        })?,
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            trimmed
                .parse::<u64>()
                .map_err(|_| CliCoreError::InvalidInput {
                    message: format!("invalid integer value for --limit: '{trimmed}'"),
                })?
        }
        _ => {
            return Err(CliCoreError::InvalidInput {
                message: "--limit must be a positive integer".to_string(),
            });
        }
    };

    if limit == 0 {
        return Err(CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        });
    }

    if limit > u64::from(max_limit) {
        return Err(CliCoreError::InvalidInput {
            message: format!("--limit cannot exceed {max_limit}"),
        });
    }

    Ok(limit as u32)
}
