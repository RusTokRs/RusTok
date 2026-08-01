use rustok_api::PortError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Default upper bound for one length-prefixed Comments request or response.
pub const DEFAULT_MAX_COMMENTS_FRAME_BYTES: usize = 8 * 1024 * 1024;

pub(crate) async fn write_frame<S>(
    stream: &mut S,
    payload: &[u8],
    max_frame_bytes: usize,
) -> Result<(), PortError>
where
    S: AsyncWrite + Unpin + ?Sized,
{
    ensure_frame_size(payload.len(), max_frame_bytes)?;
    let length = u32::try_from(payload.len()).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_frame_too_large",
            "comments TCP frame length exceeds the u32 wire limit",
        )
    })?;

    stream
        .write_all(&length.to_be_bytes())
        .await
        .map_err(|error| io_error("write_length", error))?;
    stream
        .write_all(payload)
        .await
        .map_err(|error| io_error("write_payload", error))?;
    stream
        .flush()
        .await
        .map_err(|error| io_error("flush", error))?;
    Ok(())
}

pub(crate) async fn read_frame<S>(
    stream: &mut S,
    max_frame_bytes: usize,
) -> Result<Vec<u8>, PortError>
where
    S: AsyncRead + Unpin + ?Sized,
{
    let mut length_bytes = [0_u8; 4];
    stream
        .read_exact(&mut length_bytes)
        .await
        .map_err(|error| io_error("read_length", error))?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    ensure_frame_size(length, max_frame_bytes)?;

    let mut payload = vec![0_u8; length];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(|error| io_error("read_payload", error))?;
    Ok(payload)
}

pub(crate) fn validate_frame_limit(max_frame_bytes: usize) -> Result<(), PortError> {
    if max_frame_bytes == 0 || max_frame_bytes > u32::MAX as usize {
        return Err(PortError::validation(
            "comments.tcp_invalid_frame_limit",
            "comments TCP frame limit must be within 1..=u32::MAX",
        ));
    }
    Ok(())
}

pub(crate) fn ensure_frame_size(
    length: usize,
    max_frame_bytes: usize,
) -> Result<(), PortError> {
    validate_frame_limit(max_frame_bytes)?;
    if length > max_frame_bytes {
        return Err(PortError::invariant_violation(
            "comments.tcp_frame_too_large",
            format!(
                "comments TCP frame length {length} exceeds configured limit {max_frame_bytes}"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn io_error(stage: &'static str, error: std::io::Error) -> PortError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        return PortError::timeout(
            "comments.tcp_timeout",
            format!("comments TCP {stage} timed out"),
        );
    }
    PortError::unavailable(
        "comments.tcp_unavailable",
        format!("comments TCP {stage} failed: {error}"),
    )
}
