use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};

use tokio::sync::Barrier;

use super::source::BlockingSource;

pub struct SourceControl {
    pub calls: Arc<AtomicUsize>,
    pub entered: Arc<Barrier>,
    pub release: Arc<Barrier>,
    block_first: Arc<AtomicBool>,
}

impl SourceControl {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            entered: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
            block_first: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn source(&self) -> BlockingSource {
        BlockingSource {
            calls: self.calls.clone(),
            block_first: self.block_first.clone(),
            entered: self.entered.clone(),
            release: self.release.clone(),
        }
    }
}
