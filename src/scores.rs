use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_SCORES: usize = 5;

#[derive(Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub combined_mbps: f64,
    pub date: DateTime<Local>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ScoreBoard {
    pub entries: Vec<ScoreEntry>,
}

fn scores_path() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("myaku");
    dir.join("scores.json")
}

impl ScoreBoard {
    pub fn load() -> Self {
        let path = scores_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = scores_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub fn add(&mut self, download_mbps: f64, upload_mbps: f64) {
        let combined_mbps = download_mbps + upload_mbps;
        self.entries.push(ScoreEntry {
            download_mbps,
            upload_mbps,
            combined_mbps,
            date: Local::now(),
        });
        self.entries
            .sort_by(|a, b| b.combined_mbps.partial_cmp(&a.combined_mbps).unwrap());
        self.entries.truncate(MAX_SCORES);
        self.save();
    }
}
