use futures_util::StreamExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const DOWNLOAD_CHUNK_BYTES: usize = 100 * 1024 * 1024; // 100MB per request
const NUM_DOWNLOAD_STREAMS: usize = 4;
const WARMUP_DURATION: Duration = Duration::from_secs(2);
const UPLOAD_URL: &str = "https://speed.cloudflare.com/__up";
const UPLOAD_CHUNK: usize = 512 * 1024;
const REPORT_INTERVAL: Duration = Duration::from_millis(250);
const TEST_DURATION: Duration = Duration::from_secs(10);

pub enum SpeedMsg {
    Progress { current_mbps: f64 },
    PhaseComplete { avg_mbps: f64, samples: Vec<u64> },
    Error(String),
}

fn bytes_to_mbps(bytes: u64, duration: Duration) -> f64 {
    let secs = duration.as_secs_f64();
    if secs == 0.0 {
        return 0.0;
    }
    (bytes as f64 * 8.0) / secs / 1_000_000.0
}

pub async fn run_download(tx: mpsc::Sender<SpeedMsg>) {
    let client = reqwest::Client::new();
    let url = format!(
        "https://speed.cloudflare.com/__down?bytes={}",
        DOWNLOAD_CHUNK_BYTES
    );

    let total_bytes = Arc::new(AtomicU64::new(0));
    let test_start = Instant::now();

    // Spawn parallel download streams
    let mut handles = Vec::new();
    for _ in 0..NUM_DOWNLOAD_STREAMS {
        let client = client.clone();
        let url = url.clone();
        let total_bytes = Arc::clone(&total_bytes);

        handles.push(tokio::spawn(async move {
            while test_start.elapsed() < TEST_DURATION {
                let response = match client.get(&url).send().await {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let mut stream = response.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    if test_start.elapsed() >= TEST_DURATION {
                        return;
                    }
                    if let Ok(c) = chunk {
                        total_bytes.fetch_add(c.len() as u64, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    // Reporter loop: aggregate bytes from all streams and send progress
    let mut last_bytes: u64 = 0;
    let mut last_report = Instant::now();
    let mut samples: Vec<u64> = Vec::new();
    let mut warmup_bytes: Option<u64> = None;
    let mut measurement_start: Option<Instant> = None;

    while test_start.elapsed() < TEST_DURATION {
        tokio::time::sleep(REPORT_INTERVAL).await;

        let now_bytes = total_bytes.load(Ordering::Relaxed);
        let interval_bytes = now_bytes - last_bytes;
        let mbps = bytes_to_mbps(interval_bytes, last_report.elapsed());

        let _ = tx.try_send(SpeedMsg::Progress { current_mbps: mbps });

        // Collect samples only after warm-up
        if test_start.elapsed() >= WARMUP_DURATION {
            if warmup_bytes.is_none() {
                warmup_bytes = Some(now_bytes);
                measurement_start = Some(Instant::now());
            }
            samples.push((mbps * 100.0) as u64);
        }

        last_bytes = now_bytes;
        last_report = Instant::now();
    }

    for h in handles {
        let _ = h.await;
    }

    let final_total = total_bytes.load(Ordering::Relaxed);

    if final_total == 0 {
        let _ = tx
            .send(SpeedMsg::Error("Download failed: no data received".into()))
            .await;
        return;
    }

    let avg_mbps = if let (Some(wb), Some(ms)) = (warmup_bytes, measurement_start) {
        bytes_to_mbps(final_total - wb, ms.elapsed())
    } else {
        bytes_to_mbps(final_total, test_start.elapsed())
    };

    let _ = tx
        .send(SpeedMsg::PhaseComplete { avg_mbps, samples })
        .await;
}

pub async fn run_upload(tx: mpsc::Sender<SpeedMsg>) {
    let client = reqwest::Client::new();
    let payload: Vec<u8> = (0..UPLOAD_CHUNK).map(|i| (i % 256) as u8).collect();

    let test_start = Instant::now();
    let mut total_bytes: u64 = 0;
    let mut total_transfer_time = Duration::ZERO;
    let mut samples: Vec<u64> = Vec::new();

    while test_start.elapsed() < TEST_DURATION {
        let req_start = Instant::now();
        let result = client
            .post(UPLOAD_URL)
            .header("Content-Type", "application/octet-stream")
            .body(payload.clone())
            .send()
            .await;

        match result {
            Ok(_) => {
                let elapsed = req_start.elapsed();
                let mbps = bytes_to_mbps(UPLOAD_CHUNK as u64, elapsed);
                samples.push((mbps * 100.0) as u64);
                total_bytes += UPLOAD_CHUNK as u64;
                total_transfer_time += elapsed;
                let _ = tx.try_send(SpeedMsg::Progress { current_mbps: mbps });
            }
            Err(e) => {
                let _ = tx
                    .send(SpeedMsg::Error(format!("Upload request failed: {e}")))
                    .await;
                return;
            }
        }
    }

    let avg_mbps = bytes_to_mbps(total_bytes, total_transfer_time);
    let _ = tx
        .send(SpeedMsg::PhaseComplete { avg_mbps, samples })
        .await;
}
