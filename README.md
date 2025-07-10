# Screenshot Tool

A high-performance web screenshot tool written in Rust, designed for bulk URL processing with support for 100-200 concurrent screenshots.

## Features

- **High Performance**: 60+ screenshots per second throughput
- **Concurrent Processing**: Support for 100-200 concurrent URLs
- **Browser Pool Management**: Efficient Chrome headless browser pooling
- **Multiple Output Formats**: PNG, JPEG, WebP support
- **Health Monitoring**: Built-in health checks and metrics
- **CLI Interface**: Easy-to-use command-line interface

## Installation

### Prerequisites

- **Chrome/Chromium**: Required for screenshot generation
- **Rust**: Required for building from source

### From Source

```bash
git clone <repository-url>
cd screenshot-tool
cargo build --release
```

The binary will be available at `./target/release/screenshot-tool`.

## Usage

### Command Line Interface

#### Single Screenshot

```bash
./target/release/screenshot-tool single --url https://example.com --output screenshot.png

# With custom viewport
./target/release/screenshot-tool single --url https://example.com --output screenshot.png --width 1280 --height 720
```

#### Batch Processing

Create a text file with URLs (one per line):

```bash
echo "https://example.com" > urls.txt
echo "https://github.com" >> urls.txt

# Process batch
./target/release/screenshot-tool batch --input urls.txt --output screenshots/ --concurrency 50
```

#### Health Check

```bash
./target/release/screenshot-tool health --detailed
```

### Configuration

Create a configuration file:

```bash
cp config.example.json config.json
```

Example configuration:

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

## Performance Tuning

### Resource Requirements

**Minimum:**
- CPU: 2-4 cores
- Memory: 4GB RAM
- Storage: 100MB + screenshot storage

**Recommended:**
- CPU: 8+ cores
- Memory: 8GB+ RAM
- Storage: SSD with sufficient space

### Optimization Tips

1. **Concurrency**: Start with 10-20 concurrent screenshots, increase based on system resources
2. **Browser Pool**: Set pool size to 10-20 instances for optimal performance
3. **Resource Blocking**: Enable ad/tracker blocking to reduce load time
4. **Memory Management**: Monitor memory usage and adjust pool size accordingly

## Troubleshooting

### Common Issues

#### Chrome Not Found

```bash
# Specify Chrome path
screenshot-tool --chrome-path /usr/bin/google-chrome single --url https://example.com --output test.png
```

#### Memory Issues

```bash
# Reduce concurrency
screenshot-tool batch --input urls.txt --output screenshots/ --concurrency 5

# Or adjust browser pool size
screenshot-tool --pool-size 5 batch --input urls.txt --output screenshots/
```

### Debug Mode

```bash
screenshot-tool --verbose batch --input urls.txt --output screenshots/
```

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy --all-targets --all-features
```

### Formatting

```bash
cargo fmt
```