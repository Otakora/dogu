use std::{collections::HashMap, sync::{Arc, Mutex}};

use tauri::{AppHandle, Emitter};

use crate::models::JobPausedDto;

/// Decision returned from a pause prompt.
#[derive(Debug, Clone, PartialEq)]
pub enum PauseDecision {
    Retry,
    Skip,
    Abort,
}

impl PauseDecision {
    pub fn from_str(s: &str) -> Self {
        match s {
            "retry" => Self::Retry,
            "skip"  => Self::Skip,
            _       => Self::Abort,
        }
    }
}

/// Global registry mapping job_id → a one-shot channel sender.
/// When a job is paused it inserts its sender here and blocks on the receiver.
/// The `resume_job` command looks up and removes the sender, unblocking the job.
#[derive(Clone)]
pub struct PauseRegistry {
    senders: Arc<Mutex<HashMap<String, std::sync::mpsc::SyncSender<PauseDecision>>>>,
}

impl PauseRegistry {
    pub fn new() -> Self {
        Self { senders: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Called from inside a running job thread.
    /// Emits `job-paused` to the frontend and **blocks** until the user responds.
    /// Returns `Abort` if the channel is dropped before a decision arrives.
    pub fn pause_for_decision(
        &self,
        app: &AppHandle,
        job_id: &str,
        error: &str,
        file_name: &str,
        is_recoverable: bool,
    ) -> PauseDecision {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        {
            self.senders.lock().unwrap().insert(job_id.to_string(), tx);
        }
        let _ = app.emit("job-paused", JobPausedDto {
            job_id: job_id.to_string(),
            error: error.to_string(),
            file_name: file_name.to_string(),
            is_recoverable,
        });
        rx.recv().unwrap_or(PauseDecision::Abort)
    }

    /// Called from the `resume_job` Tauri command.
    pub fn resume(&self, job_id: &str, decision: PauseDecision) {
        if let Some(tx) = self.senders.lock().unwrap().remove(job_id) {
            let _ = tx.send(decision);
        }
    }

    /// Cancel a paused job — sends Abort so the waiting `wait_for_resume` returns cleanly.
    #[allow(dead_code)]
    pub fn abort_if_paused(&self, job_id: &str) {
        if let Some(tx) = self.senders.lock().unwrap().remove(job_id) {
            let _ = tx.send(PauseDecision::Abort);
        }
    }
}
