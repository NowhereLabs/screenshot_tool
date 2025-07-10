use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use screenshot_tool::{
    Config, ScreenshotRequest, Priority, BufferPool,
    RateLimiter, CircuitBreaker, MemoryMonitor, ProgressTracker,
};

#[cfg(feature = "integration_benchmarks")]
use screenshot_tool::ScreenshotService;
use std::time::Duration;
use tokio::runtime::Runtime;

fn benchmark_config_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("config");
    group.measurement_time(Duration::from_secs(1));
    
    group.bench_function("config_creation", |b| {
        b.iter(|| {
            let config = Config::default();
            black_box(config);
        });
    });
    
    group.finish();
}

fn benchmark_screenshot_request_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("screenshot_request");
    group.measurement_time(Duration::from_secs(1));
    
    group.bench_function("screenshot_request_creation", |b| {
        b.iter(|| {
            let request = ScreenshotRequest {
                url: "https://example.com".to_string(),
                priority: Priority::High,
                full_page: true,
                ..Default::default()
            };
            black_box(request);
        });
    });
    
    group.finish();
}

fn benchmark_chrome_args_generation(c: &mut Criterion) {
    let config = Config::default();
    
    c.bench_function("chrome_args_generation", |b| {
        b.iter(|| {
            let args = screenshot_tool::get_chrome_args(&config);
            black_box(args);
        });
    });
}

fn benchmark_browser_config_creation(c: &mut Criterion) {
    let config = Config::default();
    
    c.bench_function("browser_config_creation", |b| {
        b.iter(|| {
            let browser_config = screenshot_tool::create_browser_config(&config);
            black_box(browser_config);
        });
    });
}

fn benchmark_circuit_breaker(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker");
    
    // Set shorter measurement time to avoid timeouts
    group.measurement_time(Duration::from_secs(2));
    
    for failure_threshold in [5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("operations", failure_threshold),
            failure_threshold,
            |b, &threshold| {
                let breaker = CircuitBreaker::new(threshold, Duration::from_secs(60));
                b.iter(|| {
                    let can_execute = breaker.can_execute();
                    if can_execute {
                        breaker.record_success();
                    } else {
                        breaker.record_failure();
                    }
                    black_box(can_execute);
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_buffer_pool(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("buffer_pool");
    
    // Set shorter measurement time to avoid timeouts
    group.measurement_time(Duration::from_secs(2));
    
    for buffer_size in [1024, 4096, 8192].iter() {
        group.bench_with_input(
            BenchmarkId::new("get_return_buffer", buffer_size),
            buffer_size,
            |b, &size| {
                let pool = BufferPool::new(size, 10);
                b.iter(|| {
                    rt.block_on(async {
                        let buffer = pool.get_buffer().await;
                        pool.return_buffer(buffer).await;
                    })
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_rate_limiter(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("rate_limiter");
    
    // Set shorter measurement time to avoid timeouts
    group.measurement_time(Duration::from_secs(2));
    
    for rate in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("acquire", rate),
            rate,
            |b, &requests_per_second| {
                let limiter = RateLimiter::new(requests_per_second);
                b.iter(|| {
                    rt.block_on(async {
                        let acquired = limiter.acquire().await;
                        black_box(acquired);
                    })
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_memory_monitor(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_monitor");
    
    // Set shorter measurement time to avoid timeouts
    group.measurement_time(Duration::from_secs(2));
    
    for memory_limit in [1024 * 1024, 10 * 1024 * 1024, 100 * 1024 * 1024].iter() {
        group.bench_with_input(
            BenchmarkId::new("check_memory", memory_limit),
            memory_limit,
            |b, &limit| {
                let monitor = MemoryMonitor::new(limit);
                b.iter(|| {
                    monitor.update_usage(limit / 2);
                    let status = monitor.check_memory();
                    black_box(status);
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_progress_tracker(c: &mut Criterion) {
    let mut group = c.benchmark_group("progress_tracker");
    
    // Set shorter measurement time to avoid timeouts
    group.measurement_time(Duration::from_secs(2));
    
    for total in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("record_completion", total),
            total,
            |b, &total_items| {
                let tracker = ProgressTracker::new(total_items);
                let mut counter = 0;
                b.iter(|| {
                    tracker.record_completion(counter % 10 != 0); // 10% error rate
                    counter += 1;
                    if counter >= total_items {
                        counter = 0;
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_url_validation(c: &mut Criterion) {
    let urls = vec![
        "https://example.com",
        "http://example.com/path",
        "https://subdomain.example.com:8080/path?query=value",
        "ftp://example.com",
        "invalid-url",
        "",
    ];
    
    c.bench_function("url_validation", |b| {
        b.iter(|| {
            for url in &urls {
                let result = screenshot_tool::validate_url(url);
                let _ = black_box(result);
            }
        });
    });
}

fn benchmark_filename_sanitization(c: &mut Criterion) {
    let filenames = vec![
        "normal_file.txt",
        "file with spaces.txt",
        "file/with/slashes.txt",
        "file:with:colons.txt",
        "file?with?questions.txt",
        "file<with>brackets.txt",
        "file|with|pipes.txt",
    ];
    
    c.bench_function("filename_sanitization", |b| {
        b.iter(|| {
            for filename in &filenames {
                let sanitized = screenshot_tool::sanitize_filename(filename);
                black_box(sanitized);
            }
        });
    });
}

fn benchmark_format_utilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_utilities");
    
    // Set shorter measurement time to avoid timeouts
    group.measurement_time(Duration::from_secs(2));
    
    // Duration formatting
    let durations = vec![
        Duration::from_millis(100),
        Duration::from_secs(5),
        Duration::from_secs(65),
        Duration::from_secs(3665),
    ];
    
    group.bench_function("format_duration", |b| {
        b.iter(|| {
            for duration in &durations {
                let formatted = screenshot_tool::format_duration(*duration);
                black_box(formatted);
            }
        });
    });
    
    // Bytes formatting
    let byte_sizes = vec![
        512,
        1024,
        1536,
        1048576,
        1073741824,
    ];
    
    group.bench_function("format_bytes", |b| {
        b.iter(|| {
            for size in &byte_sizes {
                let formatted = screenshot_tool::format_bytes(*size);
                black_box(formatted);
            }
        });
    });
    
    group.finish();
}

fn benchmark_request_interceptor(c: &mut Criterion) {
    let interceptor = screenshot_tool::RequestInterceptor::new();
    let test_urls = vec![
        ("https://example.com/main.js", "script"),
        ("https://googletagmanager.com/script.js", "script"),
        ("https://example.com/analytics.js", "script"),
        ("https://example.com/image.png", "image"),
        ("https://ads.example.com/ad.js", "script"),
    ];
    
    c.bench_function("request_interceptor", |b| {
        b.iter(|| {
            for (url, resource_type) in &test_urls {
                let should_block = interceptor.should_block(url, resource_type);
                black_box(should_block);
            }
        });
    });
}

// Benchmark that would require Chrome (disabled by default)
#[cfg(feature = "integration_benchmarks")]
fn benchmark_service_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("service_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = Config {
                    browser_pool_size: 1,
                    max_concurrent_screenshots: 5,
                    chrome_path: Some("/usr/sbin/chromium".to_string()),
                    ..Default::default()
                };
                
                let service = ScreenshotService::new(config).await.unwrap();
                service.shutdown().await;
                black_box(service);
            })
        });
    });
}

criterion_group!(
    benches,
    benchmark_config_creation,
    benchmark_screenshot_request_creation,
    benchmark_chrome_args_generation,
    benchmark_browser_config_creation,
    benchmark_circuit_breaker,
    benchmark_buffer_pool,
    benchmark_rate_limiter,
    benchmark_memory_monitor,
    benchmark_progress_tracker,
    benchmark_url_validation,
    benchmark_filename_sanitization,
    benchmark_format_utilities,
    benchmark_request_interceptor,
);

#[cfg(feature = "integration_benchmarks")]
criterion_group!(
    integration_benches,
    benchmark_service_creation,
);

#[cfg(feature = "integration_benchmarks")]
criterion_main!(benches, integration_benches);

#[cfg(not(feature = "integration_benchmarks"))]
criterion_main!(benches);