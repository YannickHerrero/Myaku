# Myaku

A minimal terminal UI internet speed test written in Rust.

Myaku measures your download and upload speeds directly from the terminal, with live-updating results and sparkline graphs.

## Features

- Download and upload speed measurement
- Live speed display that updates throughout each test
- Sparkline graphs showing speed variation over time
- Top 5 high score board ranked by combined download + upload speed
- Last 5 recent results history
- Persistent scores saved to `~/.local/share/myaku/scores.json`
- Uses Cloudflare's speed test endpoints (no API key required)
- Single binary, no configuration needed

## Installation

### From source

Requires [Rust](https://rustup.rs/) 1.85+.

```sh
git clone https://github.com/YannickHerrero/Myaku.git
cd Myaku
cargo build --release
```

The binary will be at `target/release/myaku`.

### Run directly

```sh
cargo run --release
```

## Usage

Launch the app:

```sh
myaku
```

| Key     | Action     |
|---------|------------|
| `Enter` | Start test |
| `q`     | Quit       |
| `Esc`   | Quit       |

The test runs for ~10 seconds per phase (download, then upload). Press `Enter` again after completion to re-run.

## How it works

- **Download**: Streams a large response from Cloudflare's speed test CDN and measures throughput over 10 seconds
- **Upload**: Sends repeated 512 KiB POST requests for 10 seconds and averages the results
- Results are displayed as a cumulative average with a sparkline showing speed variation

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client

## License

MIT
