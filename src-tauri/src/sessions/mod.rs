mod history;
mod launcher;
mod manager;
mod protocol;
#[cfg(windows)]
mod windows_job;

pub use history::{HistoryError, HistoryLimits, HistoryLoader, HistorySnapshot, HistoryTurn};
pub use launcher::{ClaudeLauncher, LaunchError, LaunchOptions, ProviderSecrets, SessionMode};
pub use manager::{
    LifecycleState, SessionError, SessionErrorCode, SessionEvent, SessionEventPayload,
    SessionHandle, SessionManager, SessionStart, WatchdogConfig, PERMISSION_RESPONSE_TIMEOUT,
};
pub use protocol::{
    MalformedProtocolLine, MalformedReason, PermissionDecision, PermissionRequest, ProtocolEvent,
    ProtocolEventKind, ProtocolMessage, UserInput,
};
