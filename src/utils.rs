use std::collections::HashSet;
use std::time::Duration;
use url::Url;

pub struct RequestInterceptor {
    pub block_ads: bool,
    pub block_trackers: bool,
    pub block_images: bool,
    pub blocked_domains: HashSet<String>,
    pub blocked_resources: HashSet<String>,
}

impl RequestInterceptor {
    pub fn new() -> Self {
        let mut blocked_domains = HashSet::new();
        
        // Common ad domains
        blocked_domains.insert("googletagmanager.com".to_string());
        blocked_domains.insert("googlesyndication.com".to_string());
        blocked_domains.insert("doubleclick.net".to_string());
        blocked_domains.insert("googleadservices.com".to_string());
        blocked_domains.insert("facebook.com".to_string());
        blocked_domains.insert("twitter.com".to_string());
        blocked_domains.insert("analytics.google.com".to_string());
        
        // Common tracker domains
        blocked_domains.insert("google-analytics.com".to_string());
        blocked_domains.insert("googletagmanager.com".to_string());
        blocked_domains.insert("hotjar.com".to_string());
        blocked_domains.insert("mixpanel.com".to_string());
        blocked_domains.insert("segment.com".to_string());
        
        let mut blocked_resources = HashSet::new();
        blocked_resources.insert("analytics".to_string());
        blocked_resources.insert("tracking".to_string());
        blocked_resources.insert("ads".to_string());
        blocked_resources.insert("advertisement".to_string());
        
        Self {
            block_ads: true,
            block_trackers: true,
            block_images: false,
            blocked_domains,
            blocked_resources,
        }
    }
    
    pub fn should_block(&self, url: &str, resource_type: &str) -> bool {
        if let Ok(parsed_url) = Url::parse(url) {
            if let Some(domain) = parsed_url.domain() {
                // Check blocked domains
                if self.blocked_domains.contains(domain) {
                    return true;
                }
                
                // Check for ad/tracker patterns in URL
                let url_lower = url.to_lowercase();
                if self.block_ads && self.contains_ad_patterns(&url_lower) {
                    return true;
                }
                
                if self.block_trackers && self.contains_tracker_patterns(&url_lower) {
                    return true;
                }
                
                // Block images if configured
                if self.block_images && resource_type == "image" {
                    return true;
                }
            }
        }
        
        false
    }
    
    fn contains_ad_patterns(&self, url: &str) -> bool {
        let ad_patterns = [
            "/ads/", "/ad/", "/advertisement/", "/advertising/",
            "googleads", "googlesyndication", "doubleclick",
            "adsystem", "adnxs", "amazon-adsystem",
        ];
        
        ad_patterns.iter().any(|pattern| url.contains(pattern))
    }
    
    fn contains_tracker_patterns(&self, url: &str) -> bool {
        let tracker_patterns = [
            "analytics", "tracking", "telemetry", "metrics",
            "hotjar", "mixpanel", "segment", "gtag",
            "facebook.com/tr", "twitter.com/i/adsct",
        ];
        
        tracker_patterns.iter().any(|pattern| url.contains(pattern))
    }
    
    pub fn add_blocked_domain(&mut self, domain: String) {
        self.blocked_domains.insert(domain);
    }
    
    pub fn remove_blocked_domain(&mut self, domain: &str) {
        self.blocked_domains.remove(domain);
    }
    
    pub fn get_blocked_domains(&self) -> &HashSet<String> {
        &self.blocked_domains
    }
}

impl Default for RequestInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BufferPool {
    buffers: tokio::sync::Mutex<Vec<Vec<u8>>>,
    buffer_size: usize,
    max_buffers: usize,
}

impl BufferPool {
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        Self {
            buffers: tokio::sync::Mutex::new(Vec::new()),
            buffer_size,
            max_buffers,
        }
    }
    
    pub async fn get_buffer(&self) -> Vec<u8> {
        let mut buffers = self.buffers.lock().await;
        buffers.pop().unwrap_or_else(|| Vec::with_capacity(self.buffer_size))
    }
    
    pub async fn return_buffer(&self, mut buffer: Vec<u8>) {
        let mut buffers = self.buffers.lock().await;
        
        if buffers.len() < self.max_buffers {
            buffer.clear();
            buffers.push(buffer);
        }
    }
    
    pub async fn get_stats(&self) -> BufferStats {
        let buffers = self.buffers.lock().await;
        BufferStats {
            available_buffers: buffers.len(),
            max_buffers: self.max_buffers,
            buffer_size: self.buffer_size,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BufferStats {
    pub available_buffers: usize,
    pub max_buffers: usize,
    pub buffer_size: usize,
}

pub struct MemoryMonitor {
    max_memory: usize,
    current_usage: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    alert_threshold: usize,
}

impl MemoryMonitor {
    pub fn new(max_memory: usize) -> Self {
        Self {
            max_memory,
            current_usage: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            alert_threshold: (max_memory as f64 * 0.8) as usize,
        }
    }
    
    pub fn check_memory(&self) -> MemoryStatus {
        let current = self.current_usage.load(std::sync::atomic::Ordering::Relaxed);
        
        if current > self.max_memory {
            MemoryStatus::Critical
        } else if current > self.alert_threshold {
            MemoryStatus::Warning
        } else {
            MemoryStatus::Normal
        }
    }
    
    pub fn update_usage(&self, usage: usize) {
        self.current_usage.store(usage, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn get_usage(&self) -> usize {
        self.current_usage.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    pub fn get_usage_percentage(&self) -> f64 {
        let current = self.current_usage.load(std::sync::atomic::Ordering::Relaxed);
        (current as f64 / self.max_memory as f64) * 100.0
    }
    
    pub fn trigger_cleanup(&self) {
        // This could trigger garbage collection or other cleanup operations
        tracing::warn!("Memory usage high, triggering cleanup");
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryStatus {
    Normal,
    Warning,
    Critical,
}

pub struct RateLimiter {
    requests_per_second: usize,
    window_size: Duration,
    request_times: tokio::sync::Mutex<Vec<std::time::Instant>>,
}

impl RateLimiter {
    pub fn new(requests_per_second: usize) -> Self {
        Self {
            requests_per_second,
            window_size: Duration::from_secs(1),
            request_times: tokio::sync::Mutex::new(Vec::new()),
        }
    }
    
    pub async fn acquire(&self) -> bool {
        let now = std::time::Instant::now();
        let mut times = self.request_times.lock().await;
        
        // Remove old requests outside the window
        times.retain(|&time| now.duration_since(time) < self.window_size);
        
        if times.len() < self.requests_per_second {
            times.push(now);
            true
        } else {
            false
        }
    }
    
    pub async fn wait_for_permit(&self) {
        while !self.acquire().await {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    
    pub async fn get_current_rate(&self) -> usize {
        let now = std::time::Instant::now();
        let times = self.request_times.lock().await;
        
        times.iter()
            .filter(|&&time| now.duration_since(time) < self.window_size)
            .count()
    }
}

pub struct ConnectionPool {
    max_connections: usize,
    active_connections: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    semaphore: tokio::sync::Semaphore,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            active_connections: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            semaphore: tokio::sync::Semaphore::new(max_connections),
        }
    }
    
    pub async fn acquire(&self) -> Result<ConnectionGuard<'_>, tokio::sync::AcquireError> {
        let permit = self.semaphore.acquire().await?;
        self.active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        Ok(ConnectionGuard {
            _permit: permit,
            active_connections: self.active_connections.clone(),
        })
    }
    
    pub fn active_count(&self) -> usize {
        self.active_connections.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    pub fn available_count(&self) -> usize {
        self.semaphore.available_permits()
    }
    
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }
}

pub struct ConnectionGuard<'a> {
    _permit: tokio::sync::SemaphorePermit<'a>,
    active_connections: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl<'a> Drop for ConnectionGuard<'a> {
    fn drop(&mut self) {
        self.active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let millis = duration.subsec_millis();
    
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else if seconds > 0 {
        format!("{}.{}s", seconds, millis / 100)
    } else {
        format!("{millis}ms")
    }
}

pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

pub fn validate_url(url: &str) -> Result<Url, url::ParseError> {
    let parsed = Url::parse(url)?;
    
    // Ensure it's HTTP or HTTPS
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        _ => Err(url::ParseError::InvalidPort),
    }
}

pub fn extract_domain(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|u| u.domain().map(|d| d.to_string()))
}

pub fn is_same_domain(url1: &str, url2: &str) -> bool {
    match (extract_domain(url1), extract_domain(url2)) {
        (Some(domain1), Some(domain2)) => domain1 == domain2,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.txt"), "test.txt");
        assert_eq!(sanitize_filename("test/file.txt"), "test_file.txt");
        assert_eq!(sanitize_filename("test:file?.txt"), "test_file_.txt");
    }
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_secs(5)), "5.0s");
        assert_eq!(format_duration(Duration::from_secs(65)), "1m 5s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    }
    
    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }
    
    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://example.com").is_ok());
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("invalid-url").is_err());
    }
    
    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("https://example.com/path"), Some("example.com".to_string()));
        assert_eq!(extract_domain("http://subdomain.example.com"), Some("subdomain.example.com".to_string()));
        assert_eq!(extract_domain("invalid-url"), None);
    }
    
    #[test]
    fn test_is_same_domain() {
        assert!(is_same_domain("https://example.com/path1", "https://example.com/path2"));
        assert!(!is_same_domain("https://example.com", "https://other.com"));
        assert!(!is_same_domain("invalid-url", "https://example.com"));
    }
}