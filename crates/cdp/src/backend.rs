// Copyright 2024 Obscura contributors (Apache-2.0)
// Modified for Wren: re-targeted to the CdpBackend trait.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::broadcast;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInfo {
    pub target_id: String,
    pub url: String,
    pub title: String,
    pub attached: bool,
}

#[derive(Debug, Clone)]
pub enum BackendEvent {
    TargetCreated(TargetInfo),
    TargetChanged(TargetInfo),
    TargetDestroyed(String),
    Domain {
        target_id: String,
        method: String,
        params: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdpFailure {
    pub code: i64,
    pub message: String,
}

#[async_trait::async_trait(?Send)]
pub trait CdpBackend {
    async fn targets(&self) -> Vec<TargetInfo>;
    async fn call(
        &self,
        target_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, CdpFailure>;
    fn events(&self) -> broadcast::Receiver<BackendEvent>;
    fn set_attached(&self, _target_id: &str, _attached: bool) {}
}

pub trait CdpBackendFactory: Send + Sync {
    fn create(&self) -> Arc<dyn CdpBackend>;
}
