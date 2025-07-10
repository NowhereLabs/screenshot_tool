use clap::Parser;
use screenshot_tool::{
    Cli, CliRunner, Config, Metrics, HealthMonitor, MetricsCollector,
    setup_logging,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let args = Cli::parse();
    
    // Setup logging
    setup_logging(args.verbose)?;
    
    info!("Starting screenshot-tool v{}", env!("CARGO_PKG_VERSION"));
    
    // Load configuration
    let config = load_config(&args).await?;
    
    // Create CLI runner
    let cli_runner = CliRunner::new(config.clone(), &args).await?;
    
    // Setup metrics and monitoring
    let metrics = Arc::new(Metrics::new());
    let metrics_collector = MetricsCollector::new(metrics.clone());
    
    // Start metrics collection
    metrics_collector.start_collection().await;
    
    // Setup health monitoring
    let _health_monitor = HealthMonitor::new(
        cli_runner.service.browser_pool.clone(),
        cli_runner.service.clone(),
        metrics.clone(),
    );
    
    // Setup graceful shutdown
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
    let _shutdown_handler = setup_shutdown_handler(shutdown_tx.clone());
    
    // Start the application based on command
    let result = tokio::select! {
        result = cli_runner.run(args.command) => {
            info!("Application completed");
            result
        }
        _ = shutdown_rx.recv() => {
            info!("Received shutdown signal");
            Ok(())
        }
    };
    
    // Graceful shutdown
    info!("Shutting down...");
    cli_runner.service.shutdown().await;
    
    if let Err(e) = result {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
    
    info!("Screenshot-tool stopped");
    Ok(())
}

async fn load_config(args: &Cli) -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = if let Some(config_path) = &args.config {
        // Load from file
        let config_content = tokio::fs::read_to_string(config_path).await?;
        serde_json::from_str(&config_content)?
    } else {
        // Use default configuration
        Config::default()
    };
    
    // Override with CLI arguments
    if let Some(pool_size) = args.pool_size {
        config.browser_pool_size = pool_size;
    }
    
    if let Some(max_concurrent) = args.max_concurrent {
        config.max_concurrent_screenshots = max_concurrent;
    }
    
    if let Some(timeout) = args.timeout {
        config.screenshot_timeout = Duration::from_secs(timeout);
    }
    
    if let Some(chrome_path) = &args.chrome_path {
        config.chrome_path = Some(chrome_path.clone());
    }
    
    // Validate configuration
    validate_config(&config)?;
    
    info!("Configuration loaded successfully");
    info!("Browser pool size: {}", config.browser_pool_size);
    info!("Max concurrent screenshots: {}", config.max_concurrent_screenshots);
    info!("Screenshot timeout: {:?}", config.screenshot_timeout);
    
    Ok(config)
}

fn validate_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    if config.browser_pool_size == 0 {
        return Err("Browser pool size must be greater than 0".into());
    }
    
    if config.max_concurrent_screenshots == 0 {
        return Err("Max concurrent screenshots must be greater than 0".into());
    }
    
    if config.screenshot_timeout.as_secs() == 0 {
        return Err("Screenshot timeout must be greater than 0".into());
    }
    
    if config.viewport.width == 0 || config.viewport.height == 0 {
        return Err("Viewport dimensions must be greater than 0".into());
    }
    
    if config.retry_attempts == 0 {
        return Err("Retry attempts must be greater than 0".into());
    }
    
    Ok(())
}

fn setup_shutdown_handler(
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("Failed to create SIGINT handler");
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to create SIGTERM handler");
        
        tokio::select! {
            _ = sigint.recv() => {
                info!("Received SIGINT");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
            }
        }
        
        let _ = shutdown_tx.send(());
    })
}

