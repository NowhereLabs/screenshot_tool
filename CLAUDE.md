# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Build
```bash
cargo build --release
```

### Test
```bash
cargo test
```

### Run Benchmarks
```bash
cargo bench
```

### Lint
```bash
cargo clippy --all-targets --all-features
```

### Format
```bash
cargo fmt
```

## Project Overview

This is a high-performance web screenshot tool written in Rust, designed for bulk URL processing with support for 100-200 concurrent screenshots. It achieves 60+ screenshots per second throughput using Chrome headless browser pooling.

## Architecture

The project follows a modular architecture with these core components:

### Core Modules (src/)
- `browser_pool.rs` - Manages a pool of Chrome headless browser instances with health monitoring and automatic restarts
- `screenshot_service.rs` - Main service orchestrating the screenshot pipeline with retry logic and error handling
- `config.rs` - Configuration management with serde serialization/deserialization
- `cli.rs` - Command-line interface using clap for batch/single screenshot operations
- `worker.rs` - Worker processes for concurrent screenshot execution
- `metrics.rs` - Performance metrics collection and monitoring
- `health.rs` - Health checking system for browser instances and overall service
- `error.rs` - Custom error types with proper error handling
- `utils.rs` - Utility functions and helpers

### Key Design Patterns
- **Browser Pool Management**: Maintains 10-20 Chrome instances for optimal performance
- **Async/Await**: Fully asynchronous processing using tokio runtime
- **Circuit Breaker**: Prevents cascading failures during high load
- **Retry Logic**: Exponential backoff retry for transient failures
- **Concurrent Processing**: Semaphore-based concurrency control
- **Health Monitoring**: Automatic instance restart and health checks

## Configuration

The tool uses JSON configuration files (see `config.example.json`):
- `browser_pool_size`: Number of Chrome instances (default: 10)
- `max_concurrent_screenshots`: Max concurrent requests (default: 200)
- `screenshot_timeout`: Timeout per screenshot (default: 30s)
- `viewport`: Screen dimensions and scaling settings
- `optimization`: Resource blocking and performance settings

## CLI Usage

### Single Screenshot
```bash
./target/release/screenshot-tool single --url https://example.com --output screenshot.png
```

### Batch Processing
```bash
./target/release/screenshot-tool batch --input urls.txt --output screenshots/ --concurrency 50
```

### Health Check
```bash
./target/release/screenshot-tool health --detailed
```

## Dependencies

- `chromiumoxide` - Chrome DevTools Protocol for browser automation
- `tokio` - Async runtime with full feature set
- `clap` - Command-line argument parsing
- `serde`/`serde_json` - Configuration serialization
- `image` - Image format conversion (PNG/JPEG/WebP)
- `metrics` - Performance monitoring
- `tracing` - Structured logging

## Performance Optimizations

- Chrome instances are reused to avoid startup overhead
- Resource blocking (ads, trackers) reduces page load time
- Concurrent processing with configurable limits
- Memory management with 1GB default limit
- Viewport-only vs full-page screenshot options

## Error Handling

The service implements comprehensive error handling:
- Retryable errors: Network timeouts, browser crashes
- Non-retryable errors: Invalid URLs, configuration issues
- Circuit breaker prevents system overload
- Graceful degradation continues processing other URLs

## Docker Support

The project includes Docker containerization:
```bash
docker build -t screenshot-tool .
docker run --rm -v $(pwd)/output:/app/screenshots screenshot-tool batch --input urls.txt --output /app/screenshots
```

## Testing

Run unit tests:
```bash
cargo test
```

Run benchmarks:
```bash
cargo bench
```

The benchmark suite in `benches/screenshot_bench.rs` tests concurrent screenshot performance.