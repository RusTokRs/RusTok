use crate::{SeoError, SeoResult};

/// Opaque proof that the host explicitly enabled the durable SEO worker runtime.
///
/// Worker execution is intentionally separated from request/operator authorization. The host may
/// construct this grant only after both the process host mode and the SEO worker switch permit
/// background execution. Application worker entry points require a reference to this value.
#[derive(Debug, Clone)]
pub struct SeoWorkerAuthorization {
    _private: (),
}

impl SeoWorkerAuthorization {
    pub fn from_runtime_config(
        host_runs_background_workers: bool,
        seo_worker_enabled: bool,
    ) -> SeoResult<Self> {
        if !host_runs_background_workers || !seo_worker_enabled {
            return Err(SeoError::PermissionDenied);
        }

        Ok(Self { _private: () })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_authorization_requires_host_mode_and_worker_switch() {
        assert!(SeoWorkerAuthorization::from_runtime_config(true, true).is_ok());
        assert!(SeoWorkerAuthorization::from_runtime_config(false, true).is_err());
        assert!(SeoWorkerAuthorization::from_runtime_config(true, false).is_err());
        assert!(SeoWorkerAuthorization::from_runtime_config(false, false).is_err());
    }
}
