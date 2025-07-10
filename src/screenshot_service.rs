//! Main screenshot service orchestrating the screenshot pipeline
//! 
//! This module provides the primary `ScreenshotService` that coordinates
//! browser pools, workers, and request processing for high-performance
//! screenshot operations.

use crate::{
    BrowserPool, Config, ScreenshotError, ScreenshotRequest, ScreenshotResult,
    ScreenshotMetadata, OutputFormat, Priority, RetryConfig, CircuitBreaker,
};
// use chromiumoxide::browser::Browser;
use chromiumoxide::page::{Page, ScreenshotParams};
use chromiumoxide::handler::viewport::Viewport as ChromeViewport;
use futures::future::try_join_all;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

/// High-performance screenshot service with browser pool management
/// 
/// The main service that orchestrates the entire screenshot pipeline,
/// managing browser pools, concurrency control, and error handling.
/// 
/// # Examples
/// 
/// ```rust,no_run
/// use screenshot_tool::{Config, ScreenshotService, ScreenshotRequest};
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = Config::default();
///     let service = ScreenshotService::new(config).await?;
///     
///     let request = ScreenshotRequest {
///         url: "https://example.com".to_string(),
///         ..Default::default()
///     };
///     let result = service.screenshot_single(request).await?;
///     println!("Captured {} bytes", result.data.len());
///     
///     service.shutdown().await;
///     Ok(())
/// }
/// ```
pub struct ScreenshotService {
    pub browser_pool: Arc<BrowserPool>,
    config: Config,
    url_queue: Arc<Mutex<VecDeque<ScreenshotRequest>>>,
    circuit_breaker: Arc<CircuitBreaker>,
    concurrency_limiter: Arc<Semaphore>,
    retry_config: RetryConfig,
}

impl ScreenshotService {
    pub async fn new(config: Config) -> Result<Self, ScreenshotError> {
        let browser_pool = Arc::new(BrowserPool::new(config.clone()).await?);
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, Duration::from_secs(30)));
        let concurrency_limiter = Arc::new(Semaphore::new(config.max_concurrent_screenshots));
        
        Ok(Self {
            browser_pool,
            config,
            url_queue: Arc::new(Mutex::new(VecDeque::new())),
            circuit_breaker,
            concurrency_limiter,
            retry_config: RetryConfig::default(),
        })
    }
    
    pub async fn screenshot_urls(&self, urls: Vec<String>) -> Result<Vec<ScreenshotResult>, ScreenshotError> {
        let requests: Vec<ScreenshotRequest> = urls.into_iter()
            .map(|url| ScreenshotRequest {
                url,
                ..Default::default()
            })
            .collect();
        
        self.process_requests(requests).await
    }
    
    pub async fn screenshot_single(&self, request: ScreenshotRequest) -> Result<ScreenshotResult, ScreenshotError> {
        let results = self.process_requests(vec![request]).await?;
        results.into_iter().next()
            .ok_or(ScreenshotError::CaptureFailed("No result returned".to_string()))
    }
    
    pub async fn process_requests(&self, requests: Vec<ScreenshotRequest>) -> Result<Vec<ScreenshotResult>, ScreenshotError> {
        // Sort requests by priority
        let mut sorted_requests = requests;
        sorted_requests.sort_by(|a, b| self.priority_to_value(&b.priority).cmp(&self.priority_to_value(&a.priority)));
        
        // Process requests concurrently
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_screenshots));
        let tasks: Vec<_> = sorted_requests.into_iter().map(|request| {
            let service = self.clone();
            let semaphore = semaphore.clone();
            
            tokio::spawn(async move {
                let _permit = semaphore.acquire().await?;
                service.take_screenshot_with_retry(request).await
            })
        }).collect();
        
        let results = try_join_all(tasks).await
            .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
        
        results.into_iter().collect::<Result<Vec<_>, _>>()
    }
    
    async fn take_screenshot_with_retry(&self, mut request: ScreenshotRequest) -> Result<ScreenshotResult, ScreenshotError> {
        let mut last_error = None;
        
        for attempt in 0..self.retry_config.max_attempts {
            if !self.circuit_breaker.can_execute() {
                return Err(ScreenshotError::BrowserUnavailable);
            }
            
            request.retry_count = attempt;
            
            match self.take_screenshot(request.clone()).await {
                Ok(mut result) => {
                    self.circuit_breaker.record_success();
                    result.success = true;
                    return Ok(result);
                }
                Err(e) => {
                    last_error = Some(e.clone());
                    self.circuit_breaker.record_failure();
                    
                    if !e.is_retryable() || attempt == self.retry_config.max_attempts - 1 {
                        break;
                    }
                    
                    let delay = self.calculate_retry_delay(attempt);
                    debug!("Retrying screenshot for {} after {:?} (attempt {}/{})", 
                           request.url, delay, attempt + 1, self.retry_config.max_attempts);
                    sleep(delay).await;
                }
            }
        }
        
        // Return failed result
        Ok(ScreenshotResult {
            request_id: request.id,
            url: request.url,
            data: Vec::new(),
            format: self.config.output_format.clone(),
            timestamp: SystemTime::now(),
            duration: Duration::from_secs(0),
            success: false,
            error: last_error,
            metadata: ScreenshotMetadata {
                viewport: self.config.viewport.clone(),
                page_title: None,
                final_url: None,
                response_status: None,
                file_size: 0,
                browser_instance_id: 0,
            },
        })
    }
    
    async fn take_screenshot(&self, request: ScreenshotRequest) -> Result<ScreenshotResult, ScreenshotError> {
        let start_time = Instant::now();
        
        // Validate URL
        if !self.is_valid_url(&request.url) {
            return Err(ScreenshotError::InvalidUrl(request.url.clone()));
        }
        
        // Get browser instance
        let browser_handle = self.browser_pool.get_browser().await?;
        let browser_instance_id = browser_handle.instance_id;
        
        // Create new page
        let browser = browser_handle.browser.lock().await;
        let page = browser.new_page(&request.url).await
            .map_err(|e| ScreenshotError::PageError(e.to_string()))?;
        
        let result = self.capture_screenshot_with_timeout(
            &page,
            &request,
            browser_instance_id,
            start_time,
        ).await;
        
        // Close page
        let _ = page.close().await;
        
        result
    }
    
    async fn capture_screenshot_with_timeout(
        &self,
        page: &Page,
        request: &ScreenshotRequest,
        browser_instance_id: usize,
        start_time: Instant,
    ) -> Result<ScreenshotResult, ScreenshotError> {
        let capture_future = self.capture_screenshot(page, request, browser_instance_id, start_time);
        
        match timeout(self.config.screenshot_timeout, capture_future).await {
            Ok(result) => result,
            Err(_) => Err(ScreenshotError::Timeout(self.config.screenshot_timeout)),
        }
    }
    
    async fn capture_screenshot(
        &self,
        page: &Page,
        request: &ScreenshotRequest,
        browser_instance_id: usize,
        start_time: Instant,
    ) -> Result<ScreenshotResult, ScreenshotError> {
        // Set viewport
        let viewport = request.custom_viewport.as_ref().unwrap_or(&self.config.viewport);
        let _chrome_viewport = ChromeViewport {
            width: viewport.width,
            height: viewport.height,
            device_scale_factor: Some(viewport.device_scale_factor),
            emulating_mobile: viewport.mobile,
            has_touch: viewport.mobile,
            is_landscape: viewport.width > viewport.height,
        };
        
        // Set viewport using Chrome DevTools Protocol
        let viewport = request.custom_viewport.as_ref().unwrap_or(&self.config.viewport);
        
        let emulation_params = chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams::builder()
            .width(viewport.width)
            .height(viewport.height)
            .device_scale_factor(viewport.device_scale_factor)
            .mobile(viewport.mobile)
            .build()
            .map_err(|e| ScreenshotError::PageError(e.to_string()))?;
        
        page.execute(emulation_params).await
            .map_err(|e| ScreenshotError::PageError(e.to_string()))?;
        
        // Navigate to URL (chromiumoxide handles this automatically during new_page)
        
        // Wait for page load
        if self.config.optimization.wait_for_network_idle {
            page.wait_for_navigation().await
                .map_err(|e| ScreenshotError::PageError(e.to_string()))?;
        }
        
        // Additional wait time if specified
        if let Some(wait_time) = request.wait_time {
            sleep(wait_time).await;
        }
        
        // Get page information
        let page_title = page.get_title().await.unwrap_or_default();
        let final_url = page.url().await.unwrap_or_else(|_| Some(request.url.clone()));
        
        // Take screenshot
        let screenshot_data = if let Some(selector) = &request.element_selector {
            self.screenshot_element(page, selector).await?
        } else if request.full_page {
            self.screenshot_full_page(page).await?
        } else {
            self.screenshot_viewport(page).await?
        };
        
        let duration = start_time.elapsed();
        
        Ok(ScreenshotResult {
            request_id: request.id.clone(),
            url: request.url.clone(),
            data: screenshot_data.clone(),
            format: self.config.output_format.clone(),
            timestamp: SystemTime::now(),
            duration,
            success: true,
            error: None,
            metadata: ScreenshotMetadata {
                viewport: viewport.clone(),
                page_title,
                final_url,
                response_status: None, // chromiumoxide doesn't expose response status easily
                file_size: screenshot_data.len(),
                browser_instance_id,
            },
        })
    }
    
    async fn screenshot_viewport(&self, page: &Page) -> Result<Vec<u8>, ScreenshotError> {
        let screenshot_params = ScreenshotParams::builder()
            .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
            .build();
        
        let png_data = page.screenshot(screenshot_params).await
            .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
        
        self.convert_image_format(png_data).await
    }
    
    async fn screenshot_full_page(&self, page: &Page) -> Result<Vec<u8>, ScreenshotError> {
        let screenshot_params = ScreenshotParams::builder()
            .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
            .full_page(true)
            .build();
        
        let png_data = page.screenshot(screenshot_params).await
            .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
        
        self.convert_image_format(png_data).await
    }
    
    async fn screenshot_element(&self, page: &Page, selector: &str) -> Result<Vec<u8>, ScreenshotError> {
        let element = page.find_element(selector).await
            .map_err(|e| ScreenshotError::ElementNotFound(e.to_string()))?;
        
        let png_data = element.screenshot(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png).await
            .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
        
        self.convert_image_format(png_data).await
    }
    
    async fn convert_image_format(&self, png_data: Vec<u8>) -> Result<Vec<u8>, ScreenshotError> {
        match self.config.output_format {
            OutputFormat::Png => Ok(png_data),
            OutputFormat::Jpeg => {
                let img = image::load_from_memory(&png_data)
                    .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
                
                let mut jpeg_data = Vec::new();
                img.write_to(&mut std::io::Cursor::new(&mut jpeg_data), image::ImageFormat::Jpeg)
                    .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
                
                Ok(jpeg_data)
            }
            OutputFormat::Webp => {
                let img = image::load_from_memory(&png_data)
                    .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
                
                let mut webp_data = Vec::new();
                img.write_to(&mut std::io::Cursor::new(&mut webp_data), image::ImageFormat::WebP)
                    .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))?;
                
                Ok(webp_data)
            }
        }
    }
    
    fn is_valid_url(&self, url: &str) -> bool {
        url::Url::parse(url).is_ok()
    }
    
    fn priority_to_value(&self, priority: &Priority) -> u8 {
        match priority {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
            Priority::Critical => 3,
        }
    }
    
    fn calculate_retry_delay(&self, attempt: usize) -> Duration {
        let delay = self.retry_config.initial_delay.as_millis() as f64 
            * self.retry_config.multiplier.powi(attempt as i32);
        
        let delay = Duration::from_millis(delay as u64);
        
        if delay > self.retry_config.max_delay {
            self.retry_config.max_delay
        } else {
            delay
        }
    }
    
    pub async fn get_queue_size(&self) -> usize {
        self.url_queue.lock().await.len()
    }
    
    pub async fn clear_queue(&self) {
        self.url_queue.lock().await.clear();
    }
    
    pub async fn shutdown(&self) {
        info!("Shutting down screenshot service...");
        self.browser_pool.shutdown().await;
        info!("Screenshot service shutdown complete");
    }
}

impl Clone for ScreenshotService {
    fn clone(&self) -> Self {
        Self {
            browser_pool: self.browser_pool.clone(),
            config: self.config.clone(),
            url_queue: self.url_queue.clone(),
            circuit_breaker: self.circuit_breaker.clone(),
            concurrency_limiter: self.concurrency_limiter.clone(),
            retry_config: self.retry_config.clone(),
        }
    }
}