use crate::{BrowserPool, HealthLevel, HealthStatus, HealthThresholds, Metrics, ScreenshotService};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use tracing::{error, info, warn};

pub struct SystemHealthChecker {
    browser_pool: Arc<BrowserPool>,
    service: Arc<ScreenshotService>,
    thresholds: HealthThresholds,
    last_check: Option<Instant>,
}

impl SystemHealthChecker {
    pub fn new(
        browser_pool: Arc<BrowserPool>,
        service: Arc<ScreenshotService>,
        _metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            browser_pool,
            service,
            thresholds: HealthThresholds::default(),
            last_check: None,
        }
    }

    pub async fn check_system_health(&mut self) -> HealthStatus {
        let start_time = Instant::now();

        let browser_health = self.check_browser_pool_health().await;
        let service_health = self.check_service_health().await;
        let resource_health = self.check_resource_health().await;

        let overall_health = self.determine_overall_health(&[
            browser_health.clone(),
            service_health.clone(),
            resource_health.clone(),
        ]);

        let check_duration = start_time.elapsed();
        self.last_check = Some(start_time);

        info!(
            "Health check completed in {:?}: {:?}",
            check_duration, overall_health
        );

        HealthStatus {
            overall: overall_health,
            performance: service_health,
            resources: resource_health,
            errors: browser_health,
            timestamp: std::time::SystemTime::now(),
        }
    }

    async fn check_browser_pool_health(&self) -> HealthLevel {
        let stats = self.browser_pool.get_stats().await;

        // Check if we have enough healthy browsers
        if stats.healthy_instances < self.thresholds.min_available_browsers {
            warn!(
                "Browser pool health critical: only {} healthy instances",
                stats.healthy_instances
            );
            return HealthLevel::Critical;
        }

        // Check failure rate
        let failure_rate = if stats.total_instances > 0 {
            stats.failed_instances as f64 / stats.total_instances as f64
        } else {
            0.0
        };

        if failure_rate > 0.5 {
            error!(
                "Browser pool health critical: failure rate {:.2}%",
                failure_rate * 100.0
            );
            return HealthLevel::Critical;
        } else if failure_rate > 0.2 {
            warn!(
                "Browser pool health warning: failure rate {:.2}%",
                failure_rate * 100.0
            );
            return HealthLevel::Warning;
        }

        // Check utilization
        let utilization = if stats.total_instances > 0 {
            stats.busy_instances as f64 / stats.total_instances as f64
        } else {
            0.0
        };

        if utilization > 0.9 {
            warn!("Browser pool high utilization: {:.2}%", utilization * 100.0);
            return HealthLevel::Warning;
        }

        HealthLevel::Healthy
    }

    async fn check_service_health(&self) -> HealthLevel {
        let queue_size = self.service.get_queue_size().await;

        // Check queue size
        if queue_size > 1000 {
            error!("Service health critical: queue size {}", queue_size);
            return HealthLevel::Critical;
        } else if queue_size > 100 {
            warn!("Service health warning: queue size {}", queue_size);
            return HealthLevel::Warning;
        }

        HealthLevel::Healthy
    }

    async fn check_resource_health(&self) -> HealthLevel {
        // Check memory usage
        if let Ok(memory_usage) = self.get_memory_usage() {
            if memory_usage > self.thresholds.max_memory_usage {
                error!(
                    "Resource health critical: memory usage {} MB",
                    memory_usage / 1024 / 1024
                );
                return HealthLevel::Critical;
            } else if memory_usage > self.thresholds.max_memory_usage * 8 / 10 {
                warn!(
                    "Resource health warning: memory usage {} MB",
                    memory_usage / 1024 / 1024
                );
                return HealthLevel::Warning;
            }
        }

        // Check disk space (simplified)
        if let Ok(disk_usage) = self.get_disk_usage() {
            if disk_usage > 0.95 {
                error!(
                    "Resource health critical: disk usage {:.2}%",
                    disk_usage * 100.0
                );
                return HealthLevel::Critical;
            } else if disk_usage > 0.85 {
                warn!(
                    "Resource health warning: disk usage {:.2}%",
                    disk_usage * 100.0
                );
                return HealthLevel::Warning;
            }
        }

        HealthLevel::Healthy
    }

    fn determine_overall_health(&self, healths: &[HealthLevel]) -> HealthLevel {
        if healths.contains(&HealthLevel::Critical) {
            HealthLevel::Critical
        } else if healths.contains(&HealthLevel::Warning) {
            HealthLevel::Warning
        } else {
            HealthLevel::Healthy
        }
    }

    fn get_memory_usage(&self) -> Result<usize, Box<dyn std::error::Error>> {
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

    fn get_disk_usage(&self) -> Result<f64, Box<dyn std::error::Error>> {
        // This is a simplified disk usage check
        // In a real implementation, you'd use system APIs
        Ok(0.1) // Return 10% as a placeholder
    }

    pub async fn start_periodic_health_checks(&mut self, interval_duration: Duration) {
        let mut interval_timer = interval(interval_duration);

        loop {
            interval_timer.tick().await;

            let health_status = self.check_system_health().await;

            // Log health status
            match health_status.overall {
                HealthLevel::Healthy => {
                    info!("System health: OK");
                }
                HealthLevel::Warning => {
                    warn!(
                        "System health: WARNING - Performance: {:?}, Resources: {:?}, Errors: {:?}",
                        health_status.performance, health_status.resources, health_status.errors
                    );
                }
                HealthLevel::Critical => {
                    error!("System health: CRITICAL - Performance: {:?}, Resources: {:?}, Errors: {:?}",
                           health_status.performance, health_status.resources, health_status.errors);
                }
            }

            // Take corrective actions if needed
            if health_status.overall == HealthLevel::Critical {
                self.handle_critical_health().await;
            }
        }
    }

    async fn handle_critical_health(&self) {
        warn!("Handling critical health status");

        // Clear queue if it's too large
        let queue_size = self.service.get_queue_size().await;
        if queue_size > 1000 {
            warn!("Clearing large queue with {} items", queue_size);
            self.service.clear_queue().await;
        }

        // Restart failed browser instances
        let health_checks = self.browser_pool.health_check().await;
        for health in health_checks {
            if matches!(
                health.status,
                crate::InstanceStatus::Failed | crate::InstanceStatus::Unresponsive
            ) {
                warn!("Restarting unhealthy browser instance {}", health.id);
                if let Err(e) = self.browser_pool.restart_instance(health.id).await {
                    error!("Failed to restart browser instance {}: {}", health.id, e);
                }
            }
        }
    }
}

pub struct HealthMonitor {
    checker: SystemHealthChecker,
    alerts: Vec<HealthAlert>,
}

impl HealthMonitor {
    pub fn new(
        browser_pool: Arc<BrowserPool>,
        service: Arc<ScreenshotService>,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            checker: SystemHealthChecker::new(browser_pool, service, metrics),
            alerts: Vec::new(),
        }
    }

    pub async fn start_monitoring(&mut self, interval: Duration) {
        let mut interval_timer = tokio::time::interval(interval);

        loop {
            interval_timer.tick().await;

            let health_status = self.checker.check_system_health().await;

            // Check for alert conditions
            self.check_alerts(&health_status).await;

            // Clean up old alerts
            self.cleanup_old_alerts();
        }
    }

    async fn check_alerts(&mut self, health_status: &HealthStatus) {
        if health_status.overall == HealthLevel::Critical {
            self.create_alert(AlertType::Critical, "System health is critical".to_string());
        }

        if health_status.resources == HealthLevel::Critical {
            self.create_alert(
                AlertType::ResourceExhaustion,
                "Resource usage is critical".to_string(),
            );
        }

        if health_status.performance == HealthLevel::Critical {
            self.create_alert(
                AlertType::PerformanceDegradation,
                "Performance is critically degraded".to_string(),
            );
        }
    }

    fn create_alert(&mut self, alert_type: AlertType, message: String) {
        let alert = HealthAlert {
            id: uuid::Uuid::new_v4().to_string(),
            alert_type,
            message,
            timestamp: std::time::SystemTime::now(),
            acknowledged: false,
        };

        error!("Health Alert [{}]: {}", alert.alert_type, alert.message);
        self.alerts.push(alert);
    }

    fn cleanup_old_alerts(&mut self) {
        let cutoff = std::time::SystemTime::now() - Duration::from_secs(24 * 60 * 60);
        self.alerts.retain(|alert| alert.timestamp > cutoff);
    }

    pub fn get_active_alerts(&self) -> Vec<&HealthAlert> {
        self.alerts.iter().filter(|a| !a.acknowledged).collect()
    }

    pub fn acknowledge_alert(&mut self, alert_id: &str) {
        if let Some(alert) = self.alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            info!("Alert {} acknowledged", alert_id);
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthAlert {
    pub id: String,
    pub alert_type: AlertType,
    pub message: String,
    pub timestamp: std::time::SystemTime,
    pub acknowledged: bool,
}

#[derive(Debug, Clone)]
pub enum AlertType {
    Critical,
    ResourceExhaustion,
    PerformanceDegradation,
    BrowserPoolFailure,
    NetworkIssue,
}

impl std::fmt::Display for AlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertType::Critical => write!(f, "CRITICAL"),
            AlertType::ResourceExhaustion => write!(f, "RESOURCE_EXHAUSTION"),
            AlertType::PerformanceDegradation => write!(f, "PERFORMANCE_DEGRADATION"),
            AlertType::BrowserPoolFailure => write!(f, "BROWSER_POOL_FAILURE"),
            AlertType::NetworkIssue => write!(f, "NETWORK_ISSUE"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub max_queue_size: usize,
    pub max_error_rate: f64,
    pub max_response_time: Duration,
    pub min_available_browsers: usize,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
            max_error_rate: 0.1, // 10%
            max_response_time: Duration::from_secs(60),
            min_available_browsers: 2,
        }
    }
}

pub struct HealthEndpoint {
    monitor: Arc<tokio::sync::Mutex<HealthMonitor>>,
}

impl HealthEndpoint {
    pub fn new(monitor: HealthMonitor) -> Self {
        Self {
            monitor: Arc::new(tokio::sync::Mutex::new(monitor)),
        }
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        let mut monitor = self.monitor.lock().await;
        monitor.checker.check_system_health().await
    }

    pub async fn get_alerts(&self) -> Vec<HealthAlert> {
        let monitor = self.monitor.lock().await;
        monitor.get_active_alerts().into_iter().cloned().collect()
    }

    pub async fn acknowledge_alert(&self, alert_id: &str) {
        let mut monitor = self.monitor.lock().await;
        monitor.acknowledge_alert(alert_id);
    }
}
