use metrics::{Counter, Gauge, Histogram};
// use metrics::{counter, gauge, histogram};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

pub struct Metrics {
    pub screenshots_taken: Counter,
    pub screenshots_failed: Counter,
    pub screenshot_duration: Histogram,
    pub browser_pool_utilization: Gauge,
    pub memory_usage: Gauge,
    pub error_count: Counter,
    pub queue_size: Gauge,
    pub active_requests: Gauge,
    pub browser_restarts: Counter,
    pub network_errors: Counter,
    pub timeout_errors: Counter,
    pub retry_count: Counter,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            screenshots_taken: Counter::noop(),
            screenshots_failed: Counter::noop(),
            screenshot_duration: Histogram::noop(),
            browser_pool_utilization: Gauge::noop(),
            memory_usage: Gauge::noop(),
            error_count: Counter::noop(),
            queue_size: Gauge::noop(),
            active_requests: Gauge::noop(),
            browser_restarts: Counter::noop(),
            network_errors: Counter::noop(),
            timeout_errors: Counter::noop(),
            retry_count: Counter::noop(),
        }
    }
    
    pub fn record_screenshot(&self, duration: Duration, success: bool) {
        if success {
            self.screenshots_taken.increment(1);
        } else {
            self.screenshots_failed.increment(1);
        }
        
        self.screenshot_duration.record(duration.as_secs_f64());
    }
    
    pub fn record_browser_usage(&self, active_instances: usize, total_instances: usize) {
        let utilization = (active_instances as f64 / total_instances as f64) * 100.0;
        self.browser_pool_utilization.set(utilization);
    }
    
    pub fn record_memory_usage(&self, bytes: usize) {
        self.memory_usage.set(bytes as f64);
    }
    
    pub fn record_error(&self, error_type: &str) {
        self.error_count.increment(1);
        
        match error_type {
            "network" => self.network_errors.increment(1),
            "timeout" => self.timeout_errors.increment(1),
            _ => {}
        }
    }
    
    pub fn record_retry(&self) {
        self.retry_count.increment(1);
    }
    
    pub fn record_browser_restart(&self) {
        self.browser_restarts.increment(1);
    }
    
    pub fn set_queue_size(&self, size: usize) {
        self.queue_size.set(size as f64);
    }
    
    pub fn set_active_requests(&self, count: usize) {
        self.active_requests.set(count as f64);
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MetricsCollector {
    metrics: Arc<Metrics>,
    start_time: Instant,
    collection_interval: Duration,
}

impl MetricsCollector {
    pub fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            metrics,
            start_time: Instant::now(),
            collection_interval: Duration::from_secs(10),
        }
    }
    
    pub async fn start_collection(&self) {
        let metrics = self.metrics.clone();
        let interval = self.collection_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                // Collect system metrics
                if let Ok(memory) = Self::get_memory_usage() {
                    metrics.record_memory_usage(memory);
                }
                
                // Log metrics summary
                info!("Metrics collection completed");
            }
        });
    }
    
    fn get_memory_usage() -> Result<usize, Box<dyn std::error::Error>> {
        // This is a simplified memory usage calculation
        // In a real implementation, you'd use system APIs or crates like `sysinfo`
        let _pid = std::process::id();
        
        // Try to read from /proc/self/status (Linux)
        if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<usize>() {
                            return Ok(kb * 1024); // Convert KB to bytes
                        }
                    }
                }
            }
        }
        
        Ok(0) // Fallback if we can't read memory usage
    }
    
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub screenshots_taken: u64,
    pub screenshots_failed: u64,
    pub average_duration: f64,
    pub browser_pool_utilization: f64,
    pub memory_usage: usize,
    pub error_count: u64,
    pub queue_size: usize,
    pub active_requests: usize,
    pub browser_restarts: u64,
    pub network_errors: u64,
    pub timeout_errors: u64,
    pub retry_count: u64,
    pub uptime: Duration,
}

pub struct PerformanceTracker {
    request_times: Arc<RwLock<Vec<Duration>>>,
    error_rates: Arc<RwLock<HashMap<String, usize>>>,
    max_samples: usize,
}

impl PerformanceTracker {
    pub fn new(_metrics: Arc<Metrics>) -> Self {
        Self {
            request_times: Arc::new(RwLock::new(Vec::new())),
            error_rates: Arc::new(RwLock::new(HashMap::new())),
            max_samples: 1000,
        }
    }
    
    pub async fn record_request_time(&self, duration: Duration) {
        let mut times = self.request_times.write().await;
        times.push(duration);
        
        if times.len() > self.max_samples {
            times.remove(0);
        }
    }
    
    pub async fn record_error_rate(&self, error_type: String) {
        let mut rates = self.error_rates.write().await;
        *rates.entry(error_type).or_insert(0) += 1;
    }
    
    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let times = self.request_times.read().await;
        let errors = self.error_rates.read().await;
        
        let total_requests = times.len();
        let avg_duration = if total_requests > 0 {
            times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / total_requests as f64
        } else {
            0.0
        };
        
        let p95_duration = if total_requests > 0 {
            let mut sorted_times = times.clone();
            sorted_times.sort();
            let p95_index = (total_requests as f64 * 0.95) as usize;
            sorted_times.get(p95_index).unwrap_or(&Duration::from_secs(0)).as_secs_f64()
        } else {
            0.0
        };
        
        let throughput = if total_requests > 0 && avg_duration > 0.0 {
            1.0 / avg_duration
        } else {
            0.0
        };
        
        PerformanceStats {
            total_requests,
            average_duration: avg_duration,
            p95_duration,
            throughput,
            error_rates: errors.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub total_requests: usize,
    pub average_duration: f64,
    pub p95_duration: f64,
    pub throughput: f64,
    pub error_rates: HashMap<String, usize>,
}

pub struct PrometheusExporter {
    port: u16,
}

impl PrometheusExporter {
    pub fn new(_metrics: Arc<Metrics>, port: u16) -> Self {
        Self { port }
    }
    
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let recorder = metrics_exporter_prometheus::PrometheusBuilder::new()
            .build_recorder();
        
        metrics::set_boxed_recorder(Box::new(recorder))?;
        
        // Start the HTTP server for metrics endpoint
        info!("Starting Prometheus metrics server on port {}", self.port);
        
        // TODO: Implement actual HTTP server
        // This would typically use a web framework to serve the /metrics endpoint
        
        Ok(())
    }
}

pub struct HealthChecker {
}

impl HealthChecker {
    pub fn new(_metrics: Arc<Metrics>) -> Self {
        Self {
        }
    }
    
    pub async fn check_health(&self) -> HealthStatus {
        let performance = self.check_performance().await;
        let resources = self.check_resources().await;
        let errors = self.check_error_rates().await;
        
        let overall_status = if performance == HealthLevel::Critical ||
                              resources == HealthLevel::Critical ||
                              errors == HealthLevel::Critical {
            HealthLevel::Critical
        } else if performance == HealthLevel::Warning ||
                  resources == HealthLevel::Warning ||
                  errors == HealthLevel::Warning {
            HealthLevel::Warning
        } else {
            HealthLevel::Healthy
        };
        
        HealthStatus {
            overall: overall_status,
            performance,
            resources,
            errors,
            timestamp: std::time::SystemTime::now(),
        }
    }
    
    async fn check_performance(&self) -> HealthLevel {
        // This would check actual performance metrics
        // For now, return healthy as a placeholder
        HealthLevel::Healthy
    }
    
    async fn check_resources(&self) -> HealthLevel {
        // This would check memory usage, browser pool status, etc.
        // For now, return healthy as a placeholder
        HealthLevel::Healthy
    }
    
    async fn check_error_rates(&self) -> HealthLevel {
        // This would check error rates against thresholds
        // For now, return healthy as a placeholder
        HealthLevel::Healthy
    }
}

#[derive(Debug, Clone)]
pub struct HealthThresholds {
    pub max_avg_duration: Duration,
    pub max_error_rate: f64,
    pub max_memory_usage: usize,
    pub min_available_browsers: usize,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            max_avg_duration: Duration::from_secs(30),
            max_error_rate: 0.05, // 5%
            max_memory_usage: 1024 * 1024 * 1024, // 1GB
            min_available_browsers: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthLevel {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub overall: HealthLevel,
    pub performance: HealthLevel,
    pub resources: HealthLevel,
    pub errors: HealthLevel,
    pub timestamp: std::time::SystemTime,
}