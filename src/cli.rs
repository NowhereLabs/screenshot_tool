use crate::{
    BatchProcessor, Config, Priority, ProgressTracker, ScreenshotRequest, ScreenshotService,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
// use tokio::io::AsyncWriteExt;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "screenshot-tool")]
#[command(about = "High-performance web screenshot tool")]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, help = "Configuration file path")]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Browser pool size")]
    pub pool_size: Option<usize>,

    #[arg(long, help = "Maximum concurrent screenshots")]
    pub max_concurrent: Option<usize>,

    #[arg(long, help = "Screenshot timeout in seconds")]
    pub timeout: Option<u64>,

    #[arg(long, help = "Enable verbose logging")]
    pub verbose: bool,

    #[arg(long, help = "Chrome executable path")]
    pub chrome_path: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Take screenshots of URLs from a file
    Batch {
        #[arg(short, long, help = "Input file containing URLs (one per line)")]
        input: PathBuf,

        #[arg(short, long, help = "Output directory for screenshots")]
        output: PathBuf,

        #[arg(short, long, default_value = "10", help = "Concurrency level")]
        concurrency: usize,

        #[arg(long, help = "Output format (png, jpeg, webp)")]
        format: Option<String>,

        #[arg(long, help = "Viewport width")]
        width: Option<u32>,

        #[arg(long, help = "Viewport height")]
        height: Option<u32>,

        #[arg(long, help = "Take full page screenshots")]
        full_page: bool,

        #[arg(long, help = "Wait time in milliseconds before taking screenshot")]
        wait: Option<u64>,

        #[arg(long, help = "Progress reporting interval in seconds")]
        progress_interval: Option<u64>,
    },

    /// Take a single screenshot
    Single {
        #[arg(short, long, help = "URL to screenshot")]
        url: String,

        #[arg(short, long, help = "Output file path")]
        output: PathBuf,

        #[arg(long, help = "Output format (png, jpeg, webp)")]
        format: Option<String>,

        #[arg(long, help = "Viewport width")]
        width: Option<u32>,

        #[arg(long, help = "Viewport height")]
        height: Option<u32>,

        #[arg(long, help = "Take full page screenshot")]
        full_page: bool,

        #[arg(long, help = "Wait time in milliseconds before taking screenshot")]
        wait: Option<u64>,

        #[arg(long, help = "CSS selector for element screenshot")]
        selector: Option<String>,

        #[arg(long, help = "Request priority (low, normal, high, critical)")]
        priority: Option<String>,
    },

    /// Start monitoring server
    Server {
        #[arg(short, long, default_value = "8080", help = "Server port")]
        port: u16,

        #[arg(long, help = "Bind address")]
        bind: Option<String>,

        #[arg(long, help = "Enable metrics endpoint")]
        metrics: bool,

        #[arg(long, help = "Enable health check endpoint")]
        health: bool,
    },

    /// Validate configuration
    Validate {
        #[arg(short, long, help = "Configuration file to validate")]
        config: PathBuf,
    },

    /// Show system information and health
    Health {
        #[arg(long, help = "Show detailed browser pool information")]
        detailed: bool,
    },
}

#[derive(Debug, Clone)]
pub struct BatchOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub concurrency: usize,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub full_page: bool,
    pub wait: Option<u64>,
    pub progress_interval: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SingleOptions {
    pub url: String,
    pub output: PathBuf,
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub full_page: bool,
    pub wait: Option<u64>,
    pub selector: Option<String>,
    pub priority: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RequestOptions {
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub full_page: bool,
    pub wait: Option<u64>,
    pub selector: Option<String>,
}

pub struct CliRunner {
    pub config: Config,
    pub service: Arc<ScreenshotService>,
}

impl CliRunner {
    pub async fn new(mut config: Config, args: &Cli) -> Result<Self, Box<dyn std::error::Error>> {
        // Override config with CLI args
        if let Some(pool_size) = args.pool_size {
            config.browser_pool_size = pool_size;
        }
        if let Some(max_concurrent) = args.max_concurrent {
            config.max_concurrent_screenshots = max_concurrent;
        }
        if let Some(timeout) = args.timeout {
            config.screenshot_timeout = std::time::Duration::from_secs(timeout);
        }
        if let Some(chrome_path) = &args.chrome_path {
            config.chrome_path = Some(chrome_path.clone());
        }

        let service = Arc::new(ScreenshotService::new(config.clone()).await?);

        Ok(Self { config, service })
    }

    pub async fn run(&self, command: Commands) -> Result<(), Box<dyn std::error::Error>> {
        match command {
            Commands::Batch {
                input,
                output,
                concurrency,
                format,
                width,
                height,
                full_page,
                wait,
                progress_interval,
            } => {
                self.run_batch(BatchOptions {
                    input,
                    output,
                    concurrency,
                    format,
                    width,
                    height,
                    full_page,
                    wait,
                    progress_interval,
                })
                .await
            }
            Commands::Single {
                url,
                output,
                format,
                width,
                height,
                full_page,
                wait,
                selector,
                priority,
            } => {
                self.run_single(SingleOptions {
                    url,
                    output,
                    format,
                    width,
                    height,
                    full_page,
                    wait,
                    selector,
                    priority,
                })
                .await
            }
            Commands::Server {
                port,
                bind,
                metrics,
                health,
            } => self.run_server(port, bind, metrics, health).await,
            Commands::Validate { config } => self.validate_config(config).await,
            Commands::Health { detailed } => self.show_health(detailed).await,
        }
    }

    pub async fn run_batch(&self, options: BatchOptions) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting batch screenshot processing");

        // Read URLs from file
        let urls = self.read_urls_from_file(&options.input).await?;
        info!(
            "Loaded {} URLs from {}",
            urls.len(),
            options.input.display()
        );

        // Create output directory
        fs::create_dir_all(&options.output).await?;

        // Create requests
        let requests = self.create_requests(
            urls,
            RequestOptions {
                format: options.format,
                width: options.width,
                height: options.height,
                full_page: options.full_page,
                wait: options.wait,
                selector: None,
            },
        )?;

        // Set up progress tracking
        let progress_tracker = Arc::new(ProgressTracker::new(requests.len()));

        // Start progress reporting task
        if let Some(interval) = options.progress_interval {
            let tracker = progress_tracker.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval));

                while !tracker.is_complete() {
                    interval.tick().await;
                    let progress = tracker.get_progress();

                    println!("Progress: {}/{} ({:.1}%) - Success: {}, Errors: {}, Rate: {:.1}/s, ETA: {:?}",
                             progress.completed, progress.total,
                             (progress.completed as f64 / progress.total as f64) * 100.0,
                             progress.success, progress.errors, progress.rate, progress.eta);
                }
            });
        }

        // Process screenshots
        let mut processor = BatchProcessor::new(self.config.clone(), self.service.clone());
        let results = processor.process_batch(requests).await;

        // Save results
        let mut success_count = 0;
        let mut error_count = 0;

        for result in results {
            progress_tracker.record_completion(result.success);

            if result.success {
                let filename = self.generate_filename(&result.url, &result.format);
                let filepath = options.output.join(filename);

                fs::write(&filepath, &result.data).await?;
                success_count += 1;

                info!("Saved screenshot: {}", filepath.display());
            } else {
                error_count += 1;
                warn!("Failed to screenshot {}: {:?}", result.url, result.error);
            }
        }

        info!(
            "Batch processing completed. Success: {}, Errors: {}",
            success_count, error_count
        );
        Ok(())
    }

    pub async fn run_single(
        &self,
        options: SingleOptions,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Taking screenshot of: {}", options.url);

        let request = self.create_single_request(
            options.url,
            RequestOptions {
                format: options.format,
                width: options.width,
                height: options.height,
                full_page: options.full_page,
                wait: options.wait,
                selector: options.selector,
            },
            options.priority,
        )?;

        let result = self.service.screenshot_single(request).await?;

        if result.success {
            // Create output directory if it doesn't exist
            if let Some(parent) = options.output.parent() {
                fs::create_dir_all(parent).await?;
            }

            fs::write(&options.output, &result.data).await?;
            info!("Screenshot saved to: {}", options.output.display());

            println!("Screenshot captured successfully:");
            println!("  URL: {}", result.url);
            println!("  Output: {}", options.output.display());
            println!("  Format: {:?}", result.format);
            println!("  Size: {} bytes", result.data.len());
            println!("  Duration: {:?}", result.duration);

            if let Some(title) = &result.metadata.page_title {
                println!("  Title: {title}");
            }
        } else {
            error!("Failed to take screenshot: {:?}", result.error);
            return Err(format!("Screenshot failed: {:?}", result.error).into());
        }

        Ok(())
    }

    pub async fn run_server(
        &self,
        port: u16,
        _bind: Option<String>,
        _metrics: bool,
        _health: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting server on port {}", port);

        // TODO: Implement HTTP server
        // This would typically use a web framework like warp or axum
        println!("Server mode not yet implemented");

        Ok(())
    }

    pub async fn validate_config(
        &self,
        config_path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("Validating configuration: {}", config_path.display());

        let config_content = fs::read_to_string(&config_path).await?;
        let config: Config = serde_json::from_str(&config_content)?;

        println!("Configuration is valid:");
        println!("  Browser pool size: {}", config.browser_pool_size);
        println!("  Max concurrent: {}", config.max_concurrent_screenshots);
        println!("  Timeout: {:?}", config.screenshot_timeout);
        println!("  Output format: {:?}", config.output_format);
        println!(
            "  Viewport: {}x{}",
            config.viewport.width, config.viewport.height
        );

        Ok(())
    }

    pub async fn show_health(&self, detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
        println!("System Health Check");
        println!("==================");

        // Browser pool health
        let pool_stats = self.service.browser_pool.get_stats().await;
        println!("Browser Pool:");
        println!("  Total instances: {}", pool_stats.total_instances);
        println!("  Healthy instances: {}", pool_stats.healthy_instances);
        println!("  Busy instances: {}", pool_stats.busy_instances);
        println!("  Failed instances: {}", pool_stats.failed_instances);
        println!("  Available instances: {}", pool_stats.available_instances);
        println!("  Total screenshots: {}", pool_stats.total_screenshots);

        if detailed {
            let health_checks = self.service.browser_pool.health_check().await;
            println!("\nDetailed Instance Health:");
            for health in health_checks {
                println!(
                    "  Instance {}: {:?} - Screenshots: {}, Age: {:?}, Idle: {:?}, Failures: {}",
                    health.id,
                    health.status,
                    health.screenshot_count,
                    health.age,
                    health.idle_time,
                    health.failure_count
                );
            }
        }

        // Queue status
        let queue_size = self.service.get_queue_size().await;
        println!("\nQueue Status:");
        println!("  Pending requests: {queue_size}");

        Ok(())
    }

    pub async fn read_urls_from_file(
        &self,
        path: &PathBuf,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path).await?;
        let urls: Vec<String> = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| line.to_string())
            .collect();

        Ok(urls)
    }

    pub fn create_requests(
        &self,
        urls: Vec<String>,
        options: RequestOptions,
    ) -> Result<Vec<ScreenshotRequest>, Box<dyn std::error::Error>> {
        let mut requests = Vec::new();

        for url in urls {
            let request = self.create_single_request(url, options.clone(), None)?;
            requests.push(request);
        }

        Ok(requests)
    }

    pub fn create_single_request(
        &self,
        url: String,
        options: RequestOptions,
        priority: Option<String>,
    ) -> Result<ScreenshotRequest, Box<dyn std::error::Error>> {
        let custom_viewport = if options.width.is_some() || options.height.is_some() {
            Some(crate::Viewport {
                width: options.width.unwrap_or(self.config.viewport.width),
                height: options.height.unwrap_or(self.config.viewport.height),
                device_scale_factor: self.config.viewport.device_scale_factor,
                mobile: self.config.viewport.mobile,
            })
        } else {
            None
        };

        let wait_time = options.wait.map(std::time::Duration::from_millis);

        let request_priority = match priority.as_deref() {
            Some("low") => Priority::Low,
            Some("normal") => Priority::Normal,
            Some("high") => Priority::High,
            Some("critical") => Priority::Critical,
            _ => Priority::Normal,
        };

        Ok(ScreenshotRequest {
            url,
            priority: request_priority,
            custom_viewport,
            wait_time,
            element_selector: options.selector,
            full_page: options.full_page,
            ..Default::default()
        })
    }

    pub fn generate_filename(&self, url: &str, format: &crate::OutputFormat) -> String {
        let sanitized = url
            .replace("https://", "")
            .replace("http://", "")
            .replace("/", "_")
            .replace("?", "_")
            .replace("&", "_")
            .replace("=", "_")
            .replace(":", "_");

        let extension = match format {
            crate::OutputFormat::Png => "png",
            crate::OutputFormat::Jpeg => "jpg",
            crate::OutputFormat::Webp => "webp",
        };

        format!("{sanitized}.{extension}")
    }
}

pub fn setup_logging(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .init();

    Ok(())
}
