use crate::{
    Config, ScreenshotError, ScreenshotRequest, ScreenshotResult, ScreenshotService,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
// use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct ScreenshotWorker {
    id: usize,
    service: Arc<ScreenshotService>,
    config: Config,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    processed_count: Arc<std::sync::atomic::AtomicUsize>,
    error_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl ScreenshotWorker {
    pub fn new(id: usize, service: Arc<ScreenshotService>, config: Config) -> Self {
        Self {
            id,
            service,
            config,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            processed_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            error_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
    
    pub async fn run(
        &self,
        mut requests: mpsc::Receiver<ScreenshotRequest>,
        results: mpsc::Sender<ScreenshotResult>,
    ) {
        info!("Starting screenshot worker {}", self.id);
        self.is_running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        while let Some(request) = requests.recv().await {
            let result = self.process_request(request).await;
            
            if result.success {
                self.processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                debug!("Worker {} successfully processed request {}", self.id, result.request_id);
            } else {
                self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                warn!("Worker {} failed to process request {}: {:?}", 
                      self.id, result.request_id, result.error);
            }
            
            if let Err(e) = results.send(result).await {
                error!("Worker {} failed to send result: {}", self.id, e);
                break;
            }
        }
        
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);
        info!("Screenshot worker {} stopped", self.id);
    }
    
    pub async fn run_with_shared_receiver(
        &self,
        requests: Arc<Mutex<mpsc::Receiver<ScreenshotRequest>>>,
        results: mpsc::Sender<ScreenshotResult>,
    ) {
        info!("Starting screenshot worker {}", self.id);
        self.is_running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        loop {
            let request = {
                let mut receiver = requests.lock().await;
                receiver.recv().await
            };
            
            match request {
                Some(request) => {
                    let result = self.process_request(request).await;
                    
                    if result.success {
                        self.processed_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        debug!("Worker {} successfully processed request {}", self.id, result.request_id);
                    } else {
                        self.error_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        warn!("Worker {} failed to process request {}: {:?}", 
                              self.id, result.request_id, result.error);
                    }
                    
                    if let Err(e) = results.send(result).await {
                        error!("Worker {} failed to send result: {}", self.id, e);
                        break;
                    }
                }
                None => break,
            }
        }
        
        self.is_running.store(false, std::sync::atomic::Ordering::Relaxed);
        info!("Screenshot worker {} stopped", self.id);
    }
    
    async fn process_request(&self, request: ScreenshotRequest) -> ScreenshotResult {
        debug!("Worker {} processing request {} for URL: {}", 
               self.id, request.id, request.url);
        
        match self.service.screenshot_single(request.clone()).await {
            Ok(result) => result,
            Err(e) => {
                error!("Worker {} failed to process request {}: {}", 
                       self.id, request.id, e);
                
                // Create error result
                ScreenshotResult {
                    request_id: request.id,
                    url: request.url,
                    data: Vec::new(),
                    format: self.config.output_format.clone(),
                    timestamp: std::time::SystemTime::now(),
                    duration: Duration::from_secs(0),
                    success: false,
                    error: Some(e),
                    metadata: crate::ScreenshotMetadata {
                        viewport: self.config.viewport.clone(),
                        page_title: None,
                        final_url: None,
                        response_status: None,
                        file_size: 0,
                        browser_instance_id: 0,
                    },
                }
            }
        }
    }
    
    pub fn get_stats(&self) -> WorkerStats {
        WorkerStats {
            id: self.id,
            is_running: self.is_running.load(std::sync::atomic::Ordering::Relaxed),
            processed_count: self.processed_count.load(std::sync::atomic::Ordering::Relaxed),
            error_count: self.error_count.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
    
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    pub fn processed_count(&self) -> usize {
        self.processed_count.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    pub fn error_count(&self) -> usize {
        self.error_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct WorkerStats {
    pub id: usize,
    pub is_running: bool,
    pub processed_count: usize,
    pub error_count: usize,
}

pub struct WorkerPool {
    workers: Vec<ScreenshotWorker>,
    request_sender: mpsc::Sender<ScreenshotRequest>,
    result_receiver: mpsc::Receiver<ScreenshotResult>,
}

impl WorkerPool {
    pub fn new(config: Config, service: Arc<ScreenshotService>) -> Self {
        let worker_count = config.browser_pool_size;
        let (request_sender, request_receiver) = mpsc::channel(1000);
        let (result_sender, result_receiver) = mpsc::channel(1000);
        
        let mut workers = Vec::new();
        
        // Create workers
        for i in 0..worker_count {
            let worker = ScreenshotWorker::new(i, service.clone(), config.clone());
            workers.push(worker);
        }
        
        // Share the receiver among workers using Arc<Mutex>
        let shared_receiver = Arc::new(Mutex::new(request_receiver));
        
        // Start worker tasks
        for worker in &workers {
            let worker_clone = worker.clone();
            let rx = shared_receiver.clone();
            let tx = result_sender.clone();
            
            tokio::spawn(async move {
                worker_clone.run_with_shared_receiver(rx, tx).await;
            });
        }
        
        Self {
            workers,
            request_sender,
            result_receiver,
        }
    }
    
    pub async fn submit_request(&self, request: ScreenshotRequest) -> Result<(), ScreenshotError> {
        self.request_sender.send(request).await
            .map_err(|e| ScreenshotError::CaptureFailed(e.to_string()))
    }
    
    pub async fn get_result(&mut self) -> Option<ScreenshotResult> {
        self.result_receiver.recv().await
    }
    
    pub fn get_worker_stats(&self) -> Vec<WorkerStats> {
        self.workers.iter().map(|w| w.get_stats()).collect()
    }
    
    pub fn total_processed(&self) -> usize {
        self.workers.iter().map(|w| w.processed_count()).sum()
    }
    
    pub fn total_errors(&self) -> usize {
        self.workers.iter().map(|w| w.error_count()).sum()
    }
    
    pub fn active_workers(&self) -> usize {
        self.workers.iter().filter(|w| w.is_running()).count()
    }
}

impl Clone for ScreenshotWorker {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            service: self.service.clone(),
            config: self.config.clone(),
            is_running: self.is_running.clone(),
            processed_count: self.processed_count.clone(),
            error_count: self.error_count.clone(),
        }
    }
}

pub struct BatchProcessor {
    worker_pool: WorkerPool,
}

impl BatchProcessor {
    pub fn new(config: Config, service: Arc<ScreenshotService>) -> Self {
        let worker_pool = WorkerPool::new(config.clone(), service);
        
        Self {
            worker_pool,
        }
    }
    
    pub async fn process_batch(&mut self, requests: Vec<ScreenshotRequest>) -> Vec<ScreenshotResult> {
        let mut results = Vec::new();
        let total_requests = requests.len();
        
        info!("Processing batch of {} requests", total_requests);
        
        // Submit all requests
        for request in requests {
            if let Err(e) = self.worker_pool.submit_request(request).await {
                error!("Failed to submit request: {}", e);
            }
        }
        
        // Collect results
        for _ in 0..total_requests {
            if let Some(result) = self.worker_pool.get_result().await {
                results.push(result);
            }
        }
        
        info!("Batch processing completed. Processed: {}, Errors: {}", 
              results.iter().filter(|r| r.success).count(),
              results.iter().filter(|r| !r.success).count());
        
        results
    }
    
    pub async fn process_urls(&mut self, urls: Vec<String>) -> Vec<ScreenshotResult> {
        let requests: Vec<ScreenshotRequest> = urls.into_iter()
            .map(|url| ScreenshotRequest {
                url,
                ..Default::default()
            })
            .collect();
        
        self.process_batch(requests).await
    }
    
    pub fn get_stats(&self) -> BatchProcessorStats {
        BatchProcessorStats {
            worker_stats: self.worker_pool.get_worker_stats(),
            total_processed: self.worker_pool.total_processed(),
            total_errors: self.worker_pool.total_errors(),
            active_workers: self.worker_pool.active_workers(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchProcessorStats {
    pub worker_stats: Vec<WorkerStats>,
    pub total_processed: usize,
    pub total_errors: usize,
    pub active_workers: usize,
}

pub struct ProgressTracker {
    total: usize,
    completed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    errors: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    start_time: std::time::Instant,
}

impl ProgressTracker {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            completed: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            errors: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            start_time: std::time::Instant::now(),
        }
    }
    
    pub fn record_completion(&self, success: bool) {
        self.completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if !success {
            self.errors.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    
    pub fn get_progress(&self) -> ProgressInfo {
        let completed = self.completed.load(std::sync::atomic::Ordering::Relaxed);
        let errors = self.errors.load(std::sync::atomic::Ordering::Relaxed);
        let elapsed = self.start_time.elapsed();
        
        ProgressInfo {
            total: self.total,
            completed,
            errors,
            success: completed - errors,
            elapsed,
            rate: if elapsed.as_secs() > 0 {
                completed as f64 / elapsed.as_secs() as f64
            } else {
                0.0
            },
            eta: if completed > 0 {
                let remaining = self.total - completed;
                let rate = completed as f64 / elapsed.as_secs() as f64;
                if rate > 0.0 {
                    Some(Duration::from_secs((remaining as f64 / rate) as u64))
                } else {
                    None
                }
            } else {
                None
            },
        }
    }
    
    pub fn is_complete(&self) -> bool {
        self.completed.load(std::sync::atomic::Ordering::Relaxed) >= self.total
    }
}

#[derive(Debug, Clone)]
pub struct ProgressInfo {
    pub total: usize,
    pub completed: usize,
    pub errors: usize,
    pub success: usize,
    pub elapsed: Duration,
    pub rate: f64,
    pub eta: Option<Duration>,
}