use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

const CAP: usize = 512;

#[derive(Clone, Debug, Serialize)]
pub struct TrafficEvent {
    pub ts: u64,
    pub method: String,
    pub path_template: String,
    pub status: u16,
    pub streaming: bool,
    pub hit_types: Vec<String>,
    pub hit_counts: Vec<u32>,
    pub latency_ms: u64,
    pub error_class: Option<String>,
    pub creator_prefix8: String,
}

#[derive(Clone, Default)]
pub struct TrafficLog {
    inner: Arc<Mutex<VecDeque<TrafficEvent>>>,
}

impl TrafficLog {
    pub fn record(&self, event: TrafficEvent) {
        let mut q = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if q.len() >= CAP {
            q.pop_front();
        }
        q.push_back(event);
    }

    pub fn snapshot(&self) -> Vec<TrafficEvent> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}

#[allow(dead_code)]
pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
