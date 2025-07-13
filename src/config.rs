//! Configuration management with serde serialization/deserialization
//!
//! This module provides all configuration structures and utilities for the screenshot tool,
//! including browser settings, optimization parameters, and output formats.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main configuration structure for the screenshot tool
///
/// Controls all aspects of the screenshot service including browser pool size,
/// concurrency limits, timeouts, and optimization settings.
///
/// # Examples
///
/// ```rust
/// use screenshot_tool::Config;
///
/// // Use default configuration
/// let config = Config::default();
///
/// // Create custom configuration
/// let config = Config {
///     browser_pool_size: 5,
///     max_concurrent_screenshots: 50,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Number of Chrome browser instances to maintain in the pool (default: 10)
    ///
    /// Higher values increase concurrency but consume more memory and system resources.
    /// Recommended range: 5-20 depending on system capabilities.
    pub browser_pool_size: usize,

    /// Maximum number of concurrent screenshot operations (default: 200)
    ///
    /// This limit prevents system overload during high-volume processing.
    /// Should be higher than browser_pool_size for optimal utilization.
    pub max_concurrent_screenshots: usize,

    /// Timeout for individual screenshot operations (default: 30 seconds)
    ///
    /// Pages that take longer than this will be marked as failed and retried
    /// according to the retry_attempts setting.
    pub screenshot_timeout: Duration,

    /// Number of retry attempts for failed screenshots (default: 3)
    ///
    /// Transient failures like network timeouts will be retried up to this limit
    /// with exponential backoff delays.
    pub retry_attempts: usize,

    /// Output image format for screenshots (default: PNG)
    pub output_format: OutputFormat,

    /// Browser viewport configuration for screenshots
    pub viewport: Viewport,

    /// Performance optimization settings
    pub optimization: OptimizationSettings,

    /// Path to Chrome/Chromium executable (default: auto-detect)
    ///
    /// If None, the tool will automatically detect the Chrome installation.
    /// Specify a custom path if using a non-standard Chrome installation.
    pub chrome_path: Option<String>,

    /// Custom User-Agent string for requests (default: Chrome default)
    ///
    /// Some websites may require specific User-Agent strings for optimal rendering.
    pub user_agent: Option<String>,

    /// Memory limit per Chrome instance in bytes (default: 1GB)
    ///
    /// Helps prevent Chrome instances from consuming excessive memory during
    /// processing of complex pages.
    pub memory_limit: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            browser_pool_size: 10,
            max_concurrent_screenshots: 200,
            screenshot_timeout: Duration::from_secs(30),
            retry_attempts: 3,
            output_format: OutputFormat::Png,
            viewport: Viewport::default(),
            optimization: OptimizationSettings::default(),
            chrome_path: None,
            user_agent: None,
            memory_limit: Some(1024 * 1024 * 1024), // 1GB
        }
    }
}

/// Browser viewport configuration for screenshots
///
/// Controls the browser window size and display characteristics used when
/// rendering pages for screenshots.
///
/// # Examples
///
/// ```rust
/// use screenshot_tool::Viewport;
///
/// // Desktop viewport (default)
/// let desktop = Viewport::default();
///
/// // Mobile viewport
/// let mobile = Viewport {
///     width: 375,
///     height: 667,
///     device_scale_factor: 2.0,
///     mobile: true,
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Viewport {
    /// Viewport width in pixels (default: 1920)
    pub width: u32,

    /// Viewport height in pixels (default: 1080)
    pub height: u32,

    /// Device pixel ratio for high-DPI displays (default: 1.0)
    ///
    /// Values > 1.0 simulate high-density displays like Retina screens.
    pub device_scale_factor: f64,

    /// Whether to emulate mobile device (default: false)
    ///
    /// Enables mobile-specific rendering behaviors and touch events.
    pub mobile: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            device_scale_factor: 1.0,
            mobile: false,
        }
    }
}

/// Performance optimization settings for screenshot rendering
///
/// Controls various browser behaviors to optimize screenshot speed and quality.
/// Blocking unnecessary resources can significantly improve performance.
///
/// # Examples
///
/// ```rust
/// use screenshot_tool::OptimizationSettings;
///
/// // High-performance settings (minimal loading)
/// let fast = OptimizationSettings {
///     block_ads: true,
///     block_trackers: true,
///     block_images: true,
///     enable_javascript: false,
///     ..Default::default()
/// };
///
/// // High-fidelity settings (full rendering)
/// let detailed = OptimizationSettings {
///     block_ads: false,
///     block_trackers: false,
///     block_images: false,
///     enable_javascript: true,
///     wait_for_network_idle: true,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OptimizationSettings {
    /// Block advertisement content (default: true)
    ///
    /// Prevents loading of known advertising networks to improve speed.
    pub block_ads: bool,

    /// Block tracking scripts and analytics (default: true)
    ///
    /// Blocks common tracking and analytics scripts to reduce load time.
    pub block_trackers: bool,

    /// Block image loading (default: false)
    ///
    /// When true, images won't be loaded, significantly reducing bandwidth.
    /// Useful for text-only screenshots or performance testing.
    pub block_images: bool,

    /// Enable JavaScript execution (default: true)
    ///
    /// JavaScript is often required for proper page rendering but can be
    /// disabled for faster static content screenshots.
    pub enable_javascript: bool,

    /// Wait for network requests to complete (default: false)
    ///
    /// When true, waits for all network activity to finish before taking
    /// the screenshot. Increases accuracy but reduces speed.
    pub wait_for_network_idle: bool,

    /// Disable CSS loading (default: false)
    ///
    /// When true, CSS stylesheets won't be loaded. Useful for extracting
    /// raw content structure without styling.
    pub disable_css: bool,

    /// Disable browser plugins (default: true)
    ///
    /// Prevents Flash, Java, and other plugins from loading to improve
    /// security and performance.
    pub disable_plugins: bool,
}

impl Default for OptimizationSettings {
    fn default() -> Self {
        Self {
            block_ads: true,
            block_trackers: true,
            block_images: false,
            enable_javascript: true,
            wait_for_network_idle: false,
            disable_css: false,
            disable_plugins: true,
        }
    }
}

/// Supported output image formats for screenshots
///
/// Each format has different characteristics:
/// - PNG: Lossless compression, larger files, best quality
/// - JPEG: Lossy compression, smaller files, good for photos
/// - WebP: Modern format with excellent compression and quality
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum OutputFormat {
    /// PNG format - lossless compression, best quality
    Png,
    /// JPEG format - lossy compression, smaller files
    Jpeg,
    /// WebP format - modern compression, good balance of size and quality
    Webp,
}

/// Priority levels for screenshot requests
///
/// Higher priority requests are processed before lower priority ones
/// when the system is under load.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Priority {
    /// Low priority - processed when system resources are available
    Low,
    /// Normal priority - standard processing order (default)
    Normal,
    /// High priority - processed before normal requests
    High,
    /// Critical priority - processed immediately with maximum resources
    Critical,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone)]
pub struct ScreenshotRequest {
    pub id: String,
    pub url: String,
    pub priority: Priority,
    pub custom_viewport: Option<Viewport>,
    pub wait_time: Option<Duration>,
    pub element_selector: Option<String>,
    pub full_page: bool,
    pub retry_count: usize,
}

impl Default for ScreenshotRequest {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            url: String::new(),
            priority: Priority::default(),
            custom_viewport: None,
            wait_time: None,
            element_selector: None,
            full_page: false,
            retry_count: 0,
        }
    }
}

#[derive(Debug)]
pub struct ScreenshotResult {
    pub request_id: String,
    pub url: String,
    pub data: Vec<u8>,
    pub format: OutputFormat,
    pub timestamp: std::time::SystemTime,
    pub duration: Duration,
    pub success: bool,
    pub error: Option<crate::error::ScreenshotError>,
    pub metadata: ScreenshotMetadata,
}

#[derive(Debug, Clone)]
pub struct ScreenshotMetadata {
    pub viewport: Viewport,
    pub page_title: Option<String>,
    pub final_url: Option<String>,
    pub response_status: Option<u16>,
    pub file_size: usize,
    pub browser_instance_id: usize,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
        }
    }
}

/// Generate Chrome command-line arguments based on configuration
///
/// Creates a comprehensive set of Chrome command-line arguments optimized
/// for headless screenshot operation with performance and security settings.
///
/// # Examples
///
/// ```rust
/// use screenshot_tool::{Config, get_chrome_args};
///
/// let config = Config::default();
/// let args = get_chrome_args(&config);
/// println!("Chrome will be launched with {} arguments", args.len());
/// ```
pub fn get_chrome_args(config: &Config) -> Vec<String> {
    get_chrome_args_with_instance_id(config, None)
}

/// Generate Chrome arguments with unique instance ID for browser pool isolation
///
/// This variant creates unique temporary directories and debugging ports for each
/// browser instance to prevent singleton conflicts in concurrent environments.
///
/// # Arguments
///
/// * `config` - The configuration settings
/// * `instance_id` - Optional unique ID for this browser instance
///
/// # Examples
///
/// ```rust
/// use screenshot_tool::{Config, get_chrome_args_with_instance_id};
///
/// let config = Config::default();
/// let args = get_chrome_args_with_instance_id(&config, Some(0));
/// // Returns args with unique temp directories for instance 0
/// ```
pub fn get_chrome_args_with_instance_id(
    config: &Config,
    instance_id: Option<usize>,
) -> Vec<String> {
    let unique_id = match instance_id {
        Some(id) => format!("{}-{}", std::process::id(), id),
        None => format!("{}-{}", std::process::id(), uuid::Uuid::new_v4()),
    };

    let mut args = vec![
        "--headless".to_string(),
        "--no-sandbox".to_string(),
        "--disable-dev-shm-usage".to_string(),
        "--disable-gpu".to_string(),
        "--disable-background-timer-throttling".to_string(),
        "--disable-backgrounding-occluded-windows".to_string(),
        "--disable-renderer-backgrounding".to_string(),
        "--disable-features=TranslateUI".to_string(),
        "--disable-extensions".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-sync".to_string(),
        "--no-first-run".to_string(),
        "--disable-web-security".to_string(),
        "--disable-process-singleton-dialog".to_string(),
        "--disable-features=ProcessSingleton".to_string(),
        "--no-process-singleton-dialog".to_string(),
        "--disable-single-process".to_string(),
        "--allow-running-insecure-content".to_string(),
        "--ignore-certificate-errors".to_string(),
        "--ignore-ssl-errors".to_string(),
        "--ignore-certificate-errors-spki-list".to_string(),
        "--ignore-certificate-errors-ssl-errors".to_string(),
        format!(
            "--window-size={},{}",
            config.viewport.width, config.viewport.height
        ),
        format!("--memory-pressure-off"),
        // Add unique user data directory to avoid singleton issues
        format!("--user-data-dir=/tmp/chromium-screenshot-{}", unique_id),
        // Add unique remote debugging port for each instance
        format!(
            "--remote-debugging-port={}",
            9222 + instance_id.unwrap_or(0)
        ),
        // Set unique temp directory to avoid chromiumoxide singleton conflicts
        format!("--temp-dir=/tmp/chromium-temp-{}", unique_id),
    ];

    if let Some(memory_limit) = config.memory_limit {
        args.push(format!(
            "--max_old_space_size={}",
            memory_limit / 1024 / 1024
        ));
    }

    if config.optimization.block_images {
        args.push("--disable-images".to_string());
    }

    if !config.optimization.enable_javascript {
        args.push("--disable-javascript".to_string());
    }

    if config.optimization.disable_plugins {
        args.push("--disable-plugins".to_string());
    }

    if config.optimization.disable_css {
        args.push("--disable-css".to_string());
    }

    if let Some(user_agent) = &config.user_agent {
        args.push(format!("--user-agent={user_agent}"));
    }

    args
}

pub fn create_browser_config(config: &Config) -> chromiumoxide::browser::BrowserConfig {
    create_browser_config_with_instance_id(config, None)
}

pub fn create_browser_config_with_instance_id(
    config: &Config,
    instance_id: Option<usize>,
) -> chromiumoxide::browser::BrowserConfig {
    use chromiumoxide::browser::BrowserConfig;

    let mut builder = BrowserConfig::builder()
        .window_size(config.viewport.width, config.viewport.height)
        .args(get_chrome_args_with_instance_id(config, instance_id));

    if let Some(chrome_path) = &config.chrome_path {
        builder = builder.chrome_executable(chrome_path);
    }

    builder
        .build()
        .unwrap_or_else(|_| BrowserConfig::with_executable("/usr/sbin/chromium"))
}
