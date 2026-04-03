use futures_util::StreamExt;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const DOWNLOAD_CHUNK: usize = 10 * 1024 * 1024; // 10MB per request
const UPLOAD_URL: &str = "https://speed.cloudflare.com/__up";
const UPLOAD_CHUNK: usize = 512 * 1024;
const NUM_CHUNKS: usize = 5;

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
        DOWNLOAD_CHUNK
    );

    let mut total_bytes: u64 = 0;
    let mut total_transfer_time = Duration::ZERO;
    let mut samples: Vec<u64> = Vec::new();

    for _ in 0..NUM_CHUNKS {
        let req_start = Instant::now();

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx
                    .send(SpeedMsg::Error(format!("Download request failed: {e}")))
                    .await;
                return;
            }
        };

        let mut chunk_bytes: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(c) => chunk_bytes += c.len() as u64,
                Err(e) => {
                    let _ = tx
                        .send(SpeedMsg::Error(format!("Download stream error: {e}")))
                        .await;
                    return;
                }
            }
        }

        let elapsed = req_start.elapsed();
        let mbps = bytes_to_mbps(chunk_bytes, elapsed);
        samples.push((mbps * 100.0) as u64);
        total_bytes += chunk_bytes;
        total_transfer_time += elapsed;
        let _ = tx.try_send(SpeedMsg::Progress { current_mbps: mbps });
    }

    let avg_mbps = bytes_to_mbps(total_bytes, total_transfer_time);
    let _ = tx
        .send(SpeedMsg::PhaseComplete { avg_mbps, samples })
        .await;
}

pub async fn run_upload(tx: mpsc::Sender<SpeedMsg>) {
    let client = reqwest::Client::new();
    let payload: Vec<u8> = (0..UPLOAD_CHUNK).map(|i| (i % 256) as u8).collect();

    let mut total_bytes: u64 = 0;
    let mut total_transfer_time = Duration::ZERO;
    let mut samples: Vec<u64> = Vec::new();

    for _ in 0..NUM_CHUNKS {
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
