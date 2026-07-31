use async_trait::async_trait;
use rustok_index::{
    IndexSource, IndexSourceFailure, IndexSourceLoadBatch, IndexSourceLoadRequest,
    IndexSourcePage, IndexSourceScanRequest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureMode {
    Permanent,
    Retryable,
}

#[derive(Clone)]
pub struct FailingSource {
    code: &'static str,
    mode: FailureMode,
}

impl FailingSource {
    pub fn new(code: &'static str, mode: FailureMode) -> Self {
        Self { code, mode }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    fn failure(&self) -> IndexSourceFailure {
        match self.mode {
            FailureMode::Permanent => IndexSourceFailure::permanent(self.code),
            FailureMode::Retryable => IndexSourceFailure::retryable(self.code),
        }
        .expect("fixture failure code must be valid")
    }
}

#[async_trait]
impl IndexSource for FailingSource {
    async fn scan(
        &self,
        _request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        Err(self.failure())
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new()).expect("empty targeted load"))
    }
}
