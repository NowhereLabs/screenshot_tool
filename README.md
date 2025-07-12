# Screenshot Tool

<div align="center">

A high-performance web screenshot tool written in Rust, designed for bulk URL processing with support for 100-200 concurrent screenshots.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)

</div>

## 🚀 Features

- **⚡ High Performance**: Achieve 60+ screenshots per second throughput
- **🔄 Concurrent Processing**: Handle 100-200 concurrent URLs simultaneously
- **🏊 Browser Pool Management**: Efficient Chrome headless browser pooling with automatic restarts
- **🎨 Multiple Output Formats**: Support for PNG, JPEG, and WebP formats
- **📊 Health Monitoring**: Built-in health checks, metrics collection, and performance monitoring
- **💻 CLI Interface**: Intuitive command-line interface for both single and batch operations
- **🛡️ Error Handling**: Comprehensive retry logic with exponential backoff
- **⚙️ Configurable**: Extensive configuration options for optimization
- **🐳 Docker Support**: Containerized deployment ready

## 📦 Installation

### Prerequisites

- **Chrome/Chromium**: Required for screenshot generation
- **Rust**: 1.70+ required for building from source

### Quick Start

```bash
# Clone the repository
git clone https://github.com/your-username/screenshot-tool.git
cd screenshot-tool

# Build the release binary
cargo build --release

# The binary will be available at:
./target/release/screenshot-tool
```

### Docker Installation

```bash
# Build the Docker image
docker build -t screenshot-tool .

# Run with Docker
docker run --rm -v $(pwd)/output:/app/screenshots screenshot-tool batch --input urls.txt --output /app/screenshots
```

## 🚀 Usage

### Command Line Interface

#### Single Screenshot

```bash
# Basic screenshot
./target/release/screenshot-tool single --url https://example.com --output screenshot.png

# Custom viewport and format
./target/release/screenshot-tool single \
  --url https://example.com \
  --output screenshot.jpg \
  --width 1280 \
  --height 720 \
  --format jpeg \
  --full-page
```

#### Batch Processing

Create a text file with URLs (one per line):

```bash
# Create URL list
cat > urls.txt << EOF
https://example.com
https://github.com
https://rust-lang.org
EOF

# Process batch with high concurrency
./target/release/screenshot-tool batch \
  --input urls.txt \
  --output screenshots/ \
  --concurrency 50 \
  --format png \
  --progress-interval 5
```

#### Health Check & Monitoring

```bash
# Basic health check
./target/release/screenshot-tool health

# Detailed health information
./target/release/screenshot-tool health --detailed
```

### ⚙️ Configuration

Create a configuration file for advanced settings:

```bash
cp config.example.json config.json
```

Example configuration with performance optimizations:

```json
{
  "browser_pool_size": 10,
  "max_concurrent_screenshots": 200,
  "screenshot_timeout": {
    "secs": 30,
    "nanos": 0
  },
  "retry_attempts": 3,
  "output_format": "Png",
  "viewport": {
    "width": 1920,
    "height": 1080,
    "device_scale_factor": 1.0,
    "mobile": false
  },
  "optimization": {
    "block_ads": true,
    "block_trackers": true,
    "block_images": false,
    "enable_javascript": true
  }
}
```

**Configuration Options:**
- `browser_pool_size`: Number of Chrome instances (10-20 recommended)
- `max_concurrent_screenshots`: Max parallel screenshots (adjust based on system resources)
- `screenshot_timeout`: Timeout per screenshot operation
- `retry_attempts`: Number of retry attempts for failed screenshots
- `optimization.block_ads/trackers`: Improves performance by blocking unnecessary resources

## CLI Options

### Global Options

- `--config`: Configuration file path
- `--pool-size`: Browser pool size
- `--max-concurrent`: Maximum concurrent screenshots
- `--timeout`: Screenshot timeout in seconds
- `--verbose`: Enable verbose logging
- `--chrome-path`: Chrome executable path

### Single Command

- `--url, -u`: URL to screenshot (required)
- `--output, -o`: Output file path (required)
- `--format`: Output format (png, jpeg, webp)
- `--width`: Viewport width
- `--height`: Viewport height
- `--full-page`: Take full page screenshot
- `--wait`: Wait time in milliseconds before taking screenshot

### Batch Command

- `--input, -i`: Input file containing URLs (one per line)
- `--output, -o`: Output directory for screenshots
- `--concurrency, -c`: Concurrency level (default: 10)
- `--format`: Output format (png, jpeg, webp)
- `--width`: Viewport width
- `--height`: Viewport height
- `--full-page`: Take full page screenshots
- `--progress-interval`: Progress reporting interval in seconds

## 🎯 Performance Tuning

### System Requirements

| Component | Minimum | Recommended | High Performance |
|-----------|---------|-------------|------------------|
| **CPU** | 2-4 cores | 8+ cores | 16+ cores |
| **Memory** | 4GB RAM | 8GB+ RAM | 16GB+ RAM |
| **Storage** | 100MB + output | SSD storage | NVMe SSD |
| **Network** | Stable internet | High bandwidth | Dedicated connection |

### Optimization Guidelines

#### 🚀 Performance Optimization
1. **Concurrency Tuning**: Start with 10-20 concurrent screenshots, scale based on:
   - Available CPU cores (2-3 screenshots per core)
   - Memory capacity (each Chrome instance uses ~100-200MB)
   - Network bandwidth and stability

2. **Browser Pool Management**: 
   - Pool size: 10-20 instances for optimal performance
   - Monitor memory usage and restart instances periodically
   - Enable health checks for automatic instance recovery

3. **Resource Optimization**:
   - Enable ad/tracker blocking (`block_ads: true, block_trackers: true`)
   - Disable images for faster loading (`block_images: true`) when content isn't needed
   - Adjust viewport size based on requirements

4. **Memory Management**:
   - Monitor system memory usage
   - Reduce pool size if memory pressure occurs
   - Use full-page screenshots sparingly (higher memory usage)

## 🔧 Troubleshooting

### Common Issues & Solutions

#### Chrome/Chromium Not Found
```bash
# Specify Chrome executable path
screenshot-tool --chrome-path /usr/bin/google-chrome single --url https://example.com --output test.png

# On macOS
screenshot-tool --chrome-path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" single --url https://example.com --output test.png

# On Windows
screenshot-tool --chrome-path "C:\Program Files\Google\Chrome\Application\chrome.exe" single --url https://example.com --output test.png
```

#### Memory/Performance Issues
```bash
# Reduce concurrency for limited resources
screenshot-tool batch --input urls.txt --output screenshots/ --concurrency 5

# Reduce browser pool size
screenshot-tool --pool-size 5 batch --input urls.txt --output screenshots/

# Enable resource blocking for faster processing
screenshot-tool --config optimized-config.json batch --input urls.txt --output screenshots/
```

#### Network/Timeout Issues
```bash
# Increase timeout for slow websites
screenshot-tool --timeout 60 batch --input urls.txt --output screenshots/

# Enable retry logic
screenshot-tool batch --input urls.txt --output screenshots/ --retry-attempts 5
```

### Debug & Logging

```bash
# Enable verbose logging
screenshot-tool --verbose batch --input urls.txt --output screenshots/

# Check health status
screenshot-tool health --detailed

# Test single URL with debug info
screenshot-tool --verbose single --url https://example.com --output test.png
```

## 🛠️ Development

### Building from Source

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Build with all features
cargo build --release --all-features
```

### Testing & Quality Assurance

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run benchmarks
cargo bench

# Lint code
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Check formatting
cargo fmt --check
```

### Architecture Overview

The project follows a modular architecture:

- **`browser_pool.rs`**: Chrome instance pool management with health monitoring
- **`screenshot_service.rs`**: Main orchestration service with retry logic
- **`config.rs`**: Configuration management and validation
- **`cli.rs`**: Command-line interface and argument parsing
- **`worker.rs`**: Concurrent worker processes for screenshot execution
- **`metrics.rs`**: Performance metrics and monitoring
- **`health.rs`**: Health checking for browser instances
- **`error.rs`**: Custom error types and error handling

### Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/amazing-feature`
3. Make your changes and add tests
4. Run the test suite: `cargo test`
5. Run linting: `cargo clippy --all-targets --all-features`
6. Format code: `cargo fmt`
7. Commit changes: `git commit -m 'Add amazing feature'`
8. Push to branch: `git push origin feature/amazing-feature`
9. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [chromiumoxide](https://github.com/mattsse/chromiumoxide) for Chrome automation
- Powered by [Tokio](https://tokio.rs/) for async runtime
- CLI powered by [clap](https://github.com/clap-rs/clap)

## 📞 Support

For issues, questions, or contributions:
- 🐛 [Report bugs](https://github.com/your-username/screenshot-tool/issues)
- 💡 [Request features](https://github.com/your-username/screenshot-tool/issues)
- 📖 [Documentation](https://github.com/your-username/screenshot-tool/wiki)

---

<div align="center">
Made with ❤️ in Rust
</div>