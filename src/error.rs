use std::time::Duration;
use thiserror::Error;
use tokio::sync::AcquireError;

#[derive(Debug, Clone, Error)]
pub enum ScreenshotError {
    #[error("Browser instance unavailable")]
    BrowserUnavailable,

    #[error("URL loading failed: {0}")]
    UrlLoadFailed(String),

    #[error("Screenshot capture failed: {0}")]
    CaptureFailed(String),

    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Browser launch failed: {0}")]
    BrowserLaunchFailed(String),

    #[error("Browser process died: {0}")]
    BrowserProcessDied(String),

    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Chrome error: {0}")]
    ChromeError(String),

    #[error("Page error: {0}")]
    PageError(String),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("Resource blocking error: {0}")]
    ResourceBlockingError(String),

    #[error("Semaphore acquire error: {0}")]
    SemaphoreError(String),
}

impl ScreenshotError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ScreenshotError::BrowserUnavailable
                | ScreenshotError::UrlLoadFailed(_)
                | ScreenshotError::NetworkError(_)
                | ScreenshotError::Timeout(_)
                | ScreenshotError::PageError(_)
                | ScreenshotError::BrowserProcessDied(_)
        )
    }

    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ScreenshotError::InvalidUrl(_) => ErrorSeverity::Low,
            ScreenshotError::ElementNotFound(_) => ErrorSeverity::Low,
            ScreenshotError::ConfigurationError(_) => ErrorSeverity::High,
            ScreenshotError::MemoryLimitExceeded => ErrorSeverity::High,
            ScreenshotError::BrowserLaunchFailed(_) => ErrorSeverity::High,
            _ => ErrorSeverity::Medium,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: std::sync::Arc<std::sync::Mutex<CircuitState>>,
    failure_threshold: usize,
    recovery_timeout: Duration,
    failure_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    last_failure_time: std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>,
}

#[derive(Debug, Clone, Copy)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, recovery_timeout: Duration) -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(CircuitState::Closed)),
            failure_threshold,
            recovery_timeout,
            failure_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            last_failure_time: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn can_execute(&self) -> bool {
        let state = *self.state.lock().unwrap();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = *self.last_failure_time.lock().unwrap() {
                    if last_failure.elapsed() > self.recovery_timeout {
                        *self.state.lock().unwrap() = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&self) {
        self.failure_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
        *self.state.lock().unwrap() = CircuitState::Closed;
        *self.last_failure_time.lock().unwrap() = None;
    }

    pub fn record_failure(&self) {
        let failures = self
            .failure_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        *self.last_failure_time.lock().unwrap() = Some(std::time::Instant::now());

        if failures >= self.failure_threshold {
            *self.state.lock().unwrap() = CircuitState::Open;
        }
    }

    pub fn get_state(&self) -> CircuitState {
        *self.state.lock().unwrap()
    }

    pub fn get_failure_count(&self) -> usize {
        self.failure_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl From<AcquireError> for ScreenshotError {
    fn from(err: AcquireError) -> Self {
        ScreenshotError::SemaphoreError(err.to_string())
    }
}

impl From<std::io::Error> for ScreenshotError {
    fn from(err: std::io::Error) -> Self {
        ScreenshotError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for ScreenshotError {
    fn from(err: serde_json::Error) -> Self {
        ScreenshotError::SerializationError(err.to_string())
    }
}
