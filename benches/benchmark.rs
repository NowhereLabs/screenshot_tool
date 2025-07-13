use criterion::{black_box, criterion_group, criterion_main, Criterion};
use screenshot_tool::{Config, Priority, ScreenshotRequest};
use std::time::Duration;

#[cfg(feature = "integration_benchmarks")]
use screenshot_tool::ScreenshotService;
#[cfg(feature = "integration_benchmarks")]
use tokio::runtime::Runtime;

// Fast settings for all benchmarks
fn configure_fast_group(group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>) {
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_millis(500));
    group.sample_size(20);
}

// === UNIT BENCHMARKS ===

fn benchmark_config_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("config");
    configure_fast_group(&mut group);

    group.bench_function("creation", |b| {
        b.iter(|| {
            let config = Config::default();
            black_box(config);
        });
    });

    group.finish();
}

fn benchmark_screenshot_request_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("screenshot_request");
    configure_fast_group(&mut group);

    group.bench_function("creation", |b| {
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

fn benchmark_url_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("url_validation");
    configure_fast_group(&mut group);

    let test_urls = vec![
        "https://example.com",
        "http://example.com/path",
        "invalid-url",
    ];

    group.bench_function("validate", |b| {
        b.iter(|| {
            for url in &test_urls {
                let result = screenshot_tool::validate_url(url);
                let _ = black_box(result);
            }
        });
    });

    group.finish();
}

fn benchmark_filename_sanitization(c: &mut Criterion) {
    let mut group = c.benchmark_group("filename_sanitization");
    configure_fast_group(&mut group);

    let test_filenames = vec![
        "normal_file.txt",
        "file with spaces.txt",
        "file/with/slashes.txt",
    ];

    group.bench_function("sanitize", |b| {
        b.iter(|| {
            for filename in &test_filenames {
                let sanitized = screenshot_tool::sanitize_filename(filename);
                black_box(sanitized);
            }
        });
    });

    group.finish();
}

fn benchmark_format_utilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("format_utilities");
    configure_fast_group(&mut group);

    let test_durations = vec![Duration::from_millis(100), Duration::from_secs(5)];
    let test_byte_sizes = vec![1024, 1048576];

    group.bench_function("format_duration", |b| {
        b.iter(|| {
            for duration in &test_durations {
                let formatted = screenshot_tool::format_duration(*duration);
                black_box(formatted);
            }
        });
    });

    group.bench_function("format_bytes", |b| {
        b.iter(|| {
            for size in &test_byte_sizes {
                let formatted = screenshot_tool::format_bytes(*size);
                black_box(formatted);
            }
        });
    });

    group.finish();
}

// === INTEGRATION BENCHMARKS (require Chrome) ===

#[cfg(feature = "integration_benchmarks")]
fn benchmark_service_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("service_creation");
    configure_fast_group(&mut group);

    group.bench_function("single_browser", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = Config {
                    browser_pool_size: 1,
                    max_concurrent_screenshots: 1,
                    chrome_path: None,
                    screenshot_timeout: Duration::from_secs(5),
                    ..Default::default()
                };

                let service = ScreenshotService::new(config).await.unwrap();
                service.shutdown().await;
                black_box(service);
            })
        });
    });

    group.finish();
}

#[cfg(feature = "integration_benchmarks")]
fn benchmark_real_world_screenshot(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("real_world_screenshot");
    configure_fast_group(&mut group);

    group.bench_function("single_url", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = Config {
                    browser_pool_size: 1,
                    max_concurrent_screenshots: 1,
                    chrome_path: None,
                    screenshot_timeout: Duration::from_secs(5),
                    ..Default::default()
                };

                let service = ScreenshotService::new(config).await.unwrap();

                let request = ScreenshotRequest {
                    url: "https://example.com".to_string(),
                    ..Default::default()
                };

                let result = service.screenshot_single(request).await;
                let success = result.is_ok();

                service.shutdown().await;
                black_box(success);
            })
        });
    });

    group.finish();
}

#[cfg(feature = "integration_benchmarks")]
fn benchmark_concurrent_screenshots(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_screenshots");
    configure_fast_group(&mut group);

    group.bench_function("concurrent_3", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = Config {
                    browser_pool_size: 2,
                    max_concurrent_screenshots: 3,
                    chrome_path: None,
                    screenshot_timeout: Duration::from_secs(5),
                    ..Default::default()
                };

                let service = ScreenshotService::new(config).await.unwrap();

                let urls = [
                    "https://example.com",
                    "https://httpbin.org/html",
                    "https://github.com",
                ];
                let requests: Vec<ScreenshotRequest> = urls
                    .iter()
                    .map(|url| ScreenshotRequest {
                        url: url.to_string(),
                        ..Default::default()
                    })
                    .collect();

                let results = service.process_requests(requests).await;
                let successful = match results {
                    Ok(results) => results.iter().filter(|r| r.success).count(),
                    Err(_) => 0,
                };

                service.shutdown().await;
                black_box(successful);
            })
        });
    });

    group.finish();
}

#[cfg(feature = "integration_benchmarks")]
fn benchmark_throughput_test(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("throughput_test");
    configure_fast_group(&mut group);

    group.bench_function("batch_5_urls", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = Config {
                    browser_pool_size: 2,
                    max_concurrent_screenshots: 5,
                    chrome_path: None,
                    screenshot_timeout: Duration::from_secs(5),
                    ..Default::default()
                };

                let service = ScreenshotService::new(config).await.unwrap();
                let start_time = std::time::Instant::now();

                let test_urls = (0..5)
                    .map(|i| format!("https://httpbin.org/uuid?id={i}"))
                    .collect::<Vec<_>>();

                let requests: Vec<ScreenshotRequest> = test_urls
                    .iter()
                    .map(|url| ScreenshotRequest {
                        url: url.clone(),
                        ..Default::default()
                    })
                    .collect();

                let results = service.process_requests(requests).await;
                let duration = start_time.elapsed();
                let (_successful, screenshots_per_second) = match results {
                    Ok(results) => {
                        let successful = results.iter().filter(|r| r.success).count();
                        let screenshots_per_second = successful as f64 / duration.as_secs_f64();
                        (successful, screenshots_per_second)
                    }
                    Err(_) => (0, 0.0),
                };

                service.shutdown().await;
                black_box(screenshots_per_second);
            })
        });
    });

    group.finish();
}

#[cfg(feature = "integration_benchmarks")]
fn benchmark_error_handling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("error_handling");
    configure_fast_group(&mut group);

    group.bench_function("mixed_urls", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = Config {
                    browser_pool_size: 1,
                    max_concurrent_screenshots: 3,
                    chrome_path: None,
                    screenshot_timeout: Duration::from_secs(5),
                    ..Default::default()
                };

                let service = ScreenshotService::new(config).await.unwrap();

                let mixed_urls = [
                    "https://example.com",
                    "https://invalid-url-that-does-not-exist.com",
                    "invalid-url-format",
                ];

                let requests: Vec<ScreenshotRequest> = mixed_urls
                    .iter()
                    .map(|url| ScreenshotRequest {
                        url: url.to_string(),
                        ..Default::default()
                    })
                    .collect();

                let results = service.process_requests(requests).await;
                let success_rate = match results {
                    Ok(results) => {
                        let successful = results.iter().filter(|r| r.success).count();
                        successful as f64 / results.len() as f64
                    }
                    Err(_) => 0.0,
                };

                service.shutdown().await;
                black_box(success_rate);
            })
        });
    });

    group.finish();
}

// === BENCHMARK GROUPS ===

criterion_group!(
    unit_benches,
    benchmark_config_creation,
    benchmark_screenshot_request_creation,
    benchmark_url_validation,
    benchmark_filename_sanitization,
    benchmark_format_utilities,
);

#[cfg(feature = "integration_benchmarks")]
criterion_group!(
    integration_benches,
    benchmark_service_creation,
    benchmark_real_world_screenshot,
    benchmark_concurrent_screenshots,
    benchmark_throughput_test,
    benchmark_error_handling,
);

#[cfg(feature = "integration_benchmarks")]
criterion_main!(unit_benches, integration_benches);

#[cfg(not(feature = "integration_benchmarks"))]
criterion_main!(unit_benches);
