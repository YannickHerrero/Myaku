use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const DOWNLOAD_URL: &str = "https://speed.cloudflare.com/__down?bytes=524288";
const UPLOAD_URL: &str = "https://speed.cloudflare.com/__up";
const UPLOAD_CHUNK: usize = 512 * 1024;
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
    let test_start = Instant::now();
    let mut total_bytes: u64 = 0;
    let mut total_transfer_time = Duration::ZERO;
    let mut samples: Vec<u64> = Vec::new();

    while test_start.elapsed() < TEST_DURATION {
        let req_start = Instant::now();
        let result = client.get(DOWNLOAD_URL).send().await;

        match result {
            Ok(resp) => {
                let body = match resp.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx
                            .send(SpeedMsg::Error(format!("Download read error: {e}")))
                            .await;
                        return;
                    }
                };
                let elapsed = req_start.elapsed();
                let mbps = bytes_to_mbps(body.len() as u64, elapsed);
                samples.push((mbps * 100.0) as u64);
                total_bytes += body.len() as u64;
                total_transfer_time += elapsed;
                let _ = tx.send(SpeedMsg::Progress { current_mbps: mbps }).await;
            }
            Err(e) => {
                let _ = tx
                    .send(SpeedMsg::Error(format!("Download request failed: {e}")))
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

pub async fn run_upload(tx: mpsc::Sender<SpeedMsg>) {
    let client = reqwest::Client::new();
    let payload: Vec<u8> = (0..UPLOAD_CHUNK).map(|i| (i % 256) as u8).collect();

    let test_start = Instant::now();
    let mut total_bytes: u64 = 0;
    let mut total_transfer_time = Duration::ZERO;
    let mut samples: Vec<u64> = Vec::new();

    // Keep uploading until we've been running for TEST_DURATION
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
                let _ = tx.send(SpeedMsg::Progress { current_mbps: mbps }).await;
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
