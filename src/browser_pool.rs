//! Browser pool management for concurrent Chrome instances
//! 
//! This module provides a managed pool of Chrome browser instances that can be
//! shared across multiple screenshot operations for optimal performance and
//! resource utilization.

use crate::{Config, ScreenshotError, create_browser_config_with_instance_id};
use chromiumoxide::browser::Browser;
use futures::StreamExt;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Current status of a browser instance in the pool
/// 
/// Tracks the health and availability of individual Chrome instances
/// for load balancing and error recovery.
#[derive(Debug, Clone, Copy)]
pub enum InstanceStatus {
    /// Instance is ready and available for use
    Healthy,
    /// Instance is currently processing a request
    Busy,
    /// Instance is not responding to commands
    Unresponsive,
    /// Instance is being restarted due to issues
    Restarting,
    /// Instance has failed and needs replacement
    Failed,
}

/// Represents a single Chrome browser instance in the pool
/// 
/// Contains the browser handle, status information, and usage statistics
/// for managing the lifecycle and health of browser instances.
#[derive(Debug)]
pub struct BrowserInstance {
    /// Unique identifier for this browser instance
    pub id: usize,
    /// Thread-safe handle to the Chrome browser
    pub browser: Arc<Mutex<Browser>>,
    /// Background task handling Chrome DevTools Protocol communication
    pub handler: tokio::task::JoinHandle<Result<(), chromiumoxide::error::CdpError>>,
    /// Timestamp of last usage for idle detection
    pub last_used: Instant,
    /// Total number of screenshots taken by this instance
    pub screenshot_count: usize,
    /// Current operational status
    pub status: InstanceStatus,
    /// When this instance was created
    pub created_at: Instant,
    /// Number of failures encountered by this instance
    pub failure_count: usize,
}

impl BrowserInstance {
    pub fn new(id: usize, browser: Browser, handler: tokio::task::JoinHandle<Result<(), chromiumoxide::error::CdpError>>) -> Self {
        Self {
            id,
            browser: Arc::new(Mutex::new(browser)),
            handler,
            last_used: Instant::now(),
            screenshot_count: 0,
            status: InstanceStatus::Healthy,
            created_at: Instant::now(),
            failure_count: 0,
        }
    }
    
    pub fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.screenshot_count += 1;
        self.status = InstanceStatus::Busy;
    }
    
    pub fn mark_available(&mut self) {
        self.status = InstanceStatus::Healthy;
    }
    
    pub fn mark_failed(&mut self) {
        self.failure_count += 1;
        self.status = InstanceStatus::Failed;
    }
    
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, InstanceStatus::Healthy)
    }
    
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
    
    pub fn idle_time(&self) -> Duration {
        self.last_used.elapsed()
    }
    
    pub async fn shutdown(self) {
        let _ = self.browser.lock().await.close().await;
        self.handler.abort();
    }
}

pub struct BrowserHandle {
    pub browser: Arc<Mutex<Browser>>,
    pub instance_id: usize,
    pool: Arc<BrowserPool>,
}

impl BrowserHandle {
    pub fn new(browser: Arc<Mutex<Browser>>, instance_id: usize, pool: Arc<BrowserPool>) -> Self {
        Self {
            browser,
            instance_id,
            pool,
        }
    }
}

impl Drop for BrowserHandle {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let instance_id = self.instance_id;
        
        tokio::spawn(async move {
            pool.return_browser(instance_id).await;
        });
    }
}

pub struct BrowserPool {
    instances: Arc<Mutex<Vec<BrowserInstance>>>,
    available: Arc<Mutex<VecDeque<usize>>>,
    semaphore: Arc<Semaphore>,
    config: Config,
    is_shutting_down: Arc<std::sync::atomic::AtomicBool>,
}

impl BrowserPool {
    pub async fn new(config: Config) -> Result<Self, ScreenshotError> {
        let pool = Self {
            instances: Arc::new(Mutex::new(Vec::new())),
            available: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(config.browser_pool_size)),
            config: config.clone(),
            is_shutting_down: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        
        // Initialize browser instances
        pool.initialize_instances().await?;
        
        // Start health check task
        pool.start_health_check_task().await;
        
        Ok(pool)
    }
    
    async fn initialize_instances(&self) -> Result<(), ScreenshotError> {
        let mut instances = self.instances.lock().await;
        let mut available = self.available.lock().await;
        
        for i in 0..self.config.browser_pool_size {
            // Add a small delay between browser launches to avoid race conditions
            if i > 0 {
                sleep(Duration::from_millis(500)).await;
            }
            
            match self.create_browser_instance(i).await {
                Ok(instance) => {
                    instances.push(instance);
                    available.push_back(i);
                    info!("Browser instance {} created successfully", i);
                }
                Err(e) => {
                    error!("Failed to create browser instance {}: {}", i, e);
                    return Err(e);
                }
            }
        }
        
        info!("Browser pool initialized with {} instances", instances.len());
        Ok(())
    }
    
    async fn create_browser_instance(&self, id: usize) -> Result<BrowserInstance, ScreenshotError> {
        // Create unique temp directories for this instance
        let temp_dir = format!("/tmp/chromium-temp-{}-{}", std::process::id(), id);
        let user_data_dir = format!("/tmp/chromium-screenshot-{}-{}", std::process::id(), id);
        let runner_dir = format!("/tmp/chromiumoxide-runner-{}", id);
        
        // Create the directories if they don't exist
        std::fs::create_dir_all(&temp_dir).map_err(|e| ScreenshotError::BrowserLaunchFailed(format!("Failed to create temp dir: {}", e)))?;
        std::fs::create_dir_all(&user_data_dir).map_err(|e| ScreenshotError::BrowserLaunchFailed(format!("Failed to create user data dir: {}", e)))?;
        std::fs::create_dir_all(&runner_dir).map_err(|e| ScreenshotError::BrowserLaunchFailed(format!("Failed to create runner dir: {}", e)))?;
        
        // Create a unique browser config for this instance
        let instance_config = create_browser_config_with_instance_id(&self.config, Some(id));
        
        // Try to launch browser with unique environment
        let (browser, mut handler) = {
            // Set environment variable for unique chromiumoxide runner directory
            std::env::set_var("TMPDIR", &runner_dir);
            let result = Browser::launch(instance_config).await;
            // Reset environment variable
            std::env::remove_var("TMPDIR");
            result
        }
        .map_err(|e| ScreenshotError::BrowserLaunchFailed(e.to_string()))?;
        
        // Start the handler in a separate task to handle Chrome DevTools Protocol communication
        // The handler implements Stream and must be polled with .next().await in a loop
        let handler_task = tokio::spawn(async move {
            loop {
                match handler.next().await {
                    Some(Ok(_)) => {
                        // Successfully processed an event from Chrome DevTools Protocol
                        continue;
                    }
                    Some(Err(e)) => {
                        tracing::error!("Handler error: {}", e);
                        return Err(e);
                    }
                    None => {
                        // Stream ended, browser probably closed
                        tracing::info!("Handler stream ended");
                        break;
                    }
                }
            }
            Ok(())
        });
        
        Ok(BrowserInstance::new(id, browser, handler_task))
    }
    
    pub async fn get_browser(&self) -> Result<BrowserHandle, ScreenshotError> {
        if self.is_shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ScreenshotError::BrowserUnavailable);
        }
        
        // Acquire semaphore permit
        let _permit = self.semaphore.acquire().await
            .map_err(|_| ScreenshotError::BrowserUnavailable)?;
        
        // Retry logic for finding a healthy instance
        for attempt in 0..3 {
            let instance_id = {
                let mut available = self.available.lock().await;
                available.pop_front()
                    .ok_or(ScreenshotError::BrowserUnavailable)?
            };
            
            let browser_result = {
                let mut instances = self.instances.lock().await;
                let instance = instances.get_mut(instance_id)
                    .ok_or(ScreenshotError::BrowserUnavailable)?;
                
                // Check instance health and handler status
                let is_healthy = instance.is_healthy() && !instance.handler.is_finished();
                
                if !is_healthy {
                    warn!("Browser instance {} unhealthy (attempt {}), attempting restart", instance_id, attempt + 1);
                    
                    // Try to restart the instance
                    match self.restart_instance_internal(instance_id).await {
                        Ok(()) => {
                            info!("Successfully restarted browser instance {}", instance_id);
                            instance.mark_used();
                            Ok(instance.browser.clone())
                        }
                        Err(e) => {
                            error!("Failed to restart browser instance {}: {}", instance_id, e);
                            // Return instance to available pool and try another one
                            self.available.lock().await.push_back(instance_id);
                            Err(e)
                        }
                    }
                } else {
                    instance.mark_used();
                    Ok(instance.browser.clone())
                }
            };
            
            match browser_result {
                Ok(browser) => {
                    return Ok(BrowserHandle::new(browser, instance_id, Arc::new(self.clone())));
                }
                Err(_) if attempt < 2 => {
                    // Try next instance
                    continue;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        
        Err(ScreenshotError::BrowserUnavailable)
    }
    
    pub async fn return_browser(&self, instance_id: usize) {
        let mut instances = self.instances.lock().await;
        let mut available = self.available.lock().await;
        
        if let Some(instance) = instances.get_mut(instance_id) {
            instance.mark_available();
            available.push_back(instance_id);
        }
    }
    
    pub async fn health_check(&self) -> Vec<InstanceHealth> {
        let instances = self.instances.lock().await;
        let mut healths = Vec::new();
        
        for instance in instances.iter() {
            let health = InstanceHealth {
                id: instance.id,
                status: instance.status,
                screenshot_count: instance.screenshot_count,
                age: instance.age(),
                idle_time: instance.idle_time(),
                failure_count: instance.failure_count,
            };
            healths.push(health);
        }
        
        healths
    }
    
    pub async fn restart_instance(&self, instance_id: usize) -> Result<(), ScreenshotError> {
        self.restart_instance_internal(instance_id).await
    }
    
    async fn restart_instance_internal(&self, instance_id: usize) -> Result<(), ScreenshotError> {
        let mut instances = self.instances.lock().await;
        
        if let Some(instance) = instances.get_mut(instance_id) {
            instance.status = InstanceStatus::Restarting;
            
            // Shutdown old browser
            let _ = instance.browser.lock().await.close().await;
            instance.handler.abort();
            
            // Create new browser instance
            match self.create_browser_instance(instance_id).await {
                Ok(new_instance) => {
                    *instance = new_instance;
                    info!("Browser instance {} restarted successfully", instance_id);
                    Ok(())
                }
                Err(e) => {
                    instance.status = InstanceStatus::Failed;
                    error!("Failed to restart browser instance {}: {}", instance_id, e);
                    Err(e)
                }
            }
        } else {
            Err(ScreenshotError::BrowserUnavailable)
        }
    }
    
    async fn start_health_check_task(&self) {
        let pool = Arc::new(self.clone());
        let is_shutting_down = self.is_shutting_down.clone();
        
        tokio::spawn(async move {
            // Staggered intervals: quick check every 15s, deep check every 60s
            let mut quick_interval = tokio::time::interval(Duration::from_secs(15));
            let mut deep_interval = tokio::time::interval(Duration::from_secs(60));
            
            while !is_shutting_down.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::select! {
                    _ = quick_interval.tick() => {
                        pool.quick_health_check().await;
                    }
                    _ = deep_interval.tick() => {
                        pool.deep_health_check().await;
                    }
                }
            }
        });
    }
    
    async fn quick_health_check(&self) {
        let instances = self.instances.lock().await;
        for instance in instances.iter() {
            // Check for crashed handlers (quick check)
            if instance.handler.is_finished() {
                warn!("Browser instance {} handler crashed, marking for restart", instance.id);
                // Note: We can't modify here due to lock, the restart will happen on next acquire
            }
            
            // Check for unresponsive instances
            if instance.idle_time() > Duration::from_secs(300) && 
               matches!(instance.status, InstanceStatus::Busy) {
                warn!("Browser instance {} unresponsive for {}s", 
                      instance.id, instance.idle_time().as_secs());
            }
        }
    }
    
    async fn deep_health_check(&self) {
        let instances_to_restart = {
            let instances = self.instances.lock().await;
            let mut restart_list = Vec::new();
            
            for instance in instances.iter() {
                let needs_restart = 
                    // Too old (1 hour)
                    instance.age() > Duration::from_secs(3600) ||
                    // Too many failures
                    instance.failure_count > 10 ||
                    // Handler crashed
                    instance.handler.is_finished() ||
                    // Stuck in unresponsive state
                    (instance.idle_time() > Duration::from_secs(600) && 
                     matches!(instance.status, InstanceStatus::Busy));
                
                if needs_restart {
                    info!("Scheduling restart for browser instance {}: age={:?}, failures={}, handler_alive={}", 
                          instance.id, instance.age(), instance.failure_count, !instance.handler.is_finished());
                    restart_list.push(instance.id);
                }
            }
            restart_list
        };
        
        // Restart problematic instances (without holding the lock)
        for instance_id in instances_to_restart {
            if let Err(e) = self.restart_instance(instance_id).await {
                error!("Failed to restart browser instance {} during health check: {}", instance_id, e);
            }
        }
    }
    
    pub async fn shutdown(&self) {
        info!("Shutting down browser pool...");
        self.is_shutting_down.store(true, std::sync::atomic::Ordering::Relaxed);
        
        // Wait for all instances to become available
        let mut retries = 0;
        while retries < 10 {
            let available_count = self.available.lock().await.len();
            if available_count == self.config.browser_pool_size {
                break;
            }
            
            sleep(Duration::from_millis(100)).await;
            retries += 1;
        }
        
        // Shutdown all browser instances
        let mut instances = self.instances.lock().await;
        for instance in instances.drain(..) {
            instance.shutdown().await;
        }
        
        info!("Browser pool shutdown complete");
    }
    
    pub async fn get_stats(&self) -> BrowserPoolStats {
        let instances = self.instances.lock().await;
        let available = self.available.lock().await;
        
        let mut healthy_count = 0;
        let mut busy_count = 0;
        let mut failed_count = 0;
        let mut total_screenshots = 0;
        
        for instance in instances.iter() {
            total_screenshots += instance.screenshot_count;
            match instance.status {
                InstanceStatus::Healthy => healthy_count += 1,
                InstanceStatus::Busy => busy_count += 1,
                InstanceStatus::Failed => failed_count += 1,
                _ => {}
            }
        }
        
        BrowserPoolStats {
            total_instances: instances.len(),
            healthy_instances: healthy_count,
            busy_instances: busy_count,
            failed_instances: failed_count,
            available_instances: available.len(),
            total_screenshots,
        }
    }
}

impl Clone for BrowserPool {
    fn clone(&self) -> Self {
        Self {
            instances: self.instances.clone(),
            available: self.available.clone(),
            semaphore: self.semaphore.clone(),
            config: self.config.clone(),
            is_shutting_down: self.is_shutting_down.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstanceHealth {
    pub id: usize,
    pub status: InstanceStatus,
    pub screenshot_count: usize,
    pub age: Duration,
    pub idle_time: Duration,
    pub failure_count: usize,
}

#[derive(Debug, Clone)]
pub struct BrowserPoolStats {
    pub total_instances: usize,
    pub healthy_instances: usize,
    pub busy_instances: usize,
    pub failed_instances: usize,
    pub available_instances: usize,
    pub total_screenshots: usize,
}