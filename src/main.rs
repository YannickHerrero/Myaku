mod scores;
mod speedtest;

use std::io::{self, stdout};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    ExecutableCommand,
};
use ratatui::{
    Frame, Terminal,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Sparkline, Table},
};
use tokio::sync::mpsc;

use crate::scores::ScoreBoard;
use crate::speedtest::SpeedMsg;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Download,
    Upload,
    Done,
}

struct TestResult {
    speed_mbps: f64,
    samples: Vec<u64>,
}

struct App {
    phase: Phase,
    live_speed_mbps: f64,
    download_result: Option<TestResult>,
    upload_result: Option<TestResult>,
    error: Option<String>,
    should_quit: bool,
    scores: ScoreBoard,
}

impl App {
    fn new() -> Self {
        Self {
            phase: Phase::Idle,
            live_speed_mbps: 0.0,
            download_result: None,
            upload_result: None,
            error: None,
            should_quit: false,
            scores: ScoreBoard::load(),
        }
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3), // title
        Constraint::Length(2), // status
        Constraint::Length(4), // download
        Constraint::Length(4), // upload
        Constraint::Length(9), // high scores
        Constraint::Min(0),   // spacer
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Title
    let title = Paragraph::new("myaku speedtest")
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" myaku "))
        .bold();
    frame.render_widget(title, chunks[0]);

    // Status
    let status_text = match app.phase {
        Phase::Idle => "Press Enter to start".into(),
        Phase::Download => format!("Testing download... {:.1} Mbps", app.live_speed_mbps),
        Phase::Upload => format!("Testing upload... {:.1} Mbps", app.live_speed_mbps),
        Phase::Done => "Test complete. Press Enter to rerun.".into(),
    };
    let status_color = match app.phase {
        Phase::Idle => Color::Gray,
        Phase::Download | Phase::Upload => Color::Yellow,
        Phase::Done => Color::Green,
    };
    let status = Paragraph::new(status_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(status_color));
    frame.render_widget(status, chunks[1]);

    // Download section
    let dl_block = Block::default().borders(Borders::ALL).title(" Download ");
    if let Some(ref result) = app.download_result {
        let inner = dl_block.inner(chunks[2]);
        frame.render_widget(dl_block, chunks[2]);

        let dl_chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(inner);

        let speed_line = Line::from(vec![
            Span::styled(
                format!("  {:.1} Mbps", result.speed_mbps),
                Style::default().fg(Color::Cyan).bold(),
            ),
        ]);
        frame.render_widget(Paragraph::new(speed_line), dl_chunks[0]);

        if !result.samples.is_empty() {
            let sparkline = Sparkline::default()
                .data(&result.samples)
                .style(Style::default().fg(Color::Cyan));
            frame.render_widget(sparkline, dl_chunks[1]);
        }
    } else if app.phase == Phase::Download {
        let inner = dl_block.inner(chunks[2]);
        frame.render_widget(dl_block, chunks[2]);
        let text = Paragraph::new(format!("  {:.1} Mbps", app.live_speed_mbps))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(text, inner);
    } else {
        let text = Paragraph::new("  —").block(dl_block).fg(Color::DarkGray);
        frame.render_widget(text, chunks[2]);
    }

    // Upload section
    let ul_block = Block::default().borders(Borders::ALL).title(" Upload ");
    if let Some(ref result) = app.upload_result {
        let inner = ul_block.inner(chunks[3]);
        frame.render_widget(ul_block, chunks[3]);

        let ul_chunks = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(inner);

        let speed_line = Line::from(vec![
            Span::styled(
                format!("  {:.1} Mbps", result.speed_mbps),
                Style::default().fg(Color::Magenta).bold(),
            ),
        ]);
        frame.render_widget(Paragraph::new(speed_line), ul_chunks[0]);

        if !result.samples.is_empty() {
            let sparkline = Sparkline::default()
                .data(&result.samples)
                .style(Style::default().fg(Color::Magenta));
            frame.render_widget(sparkline, ul_chunks[1]);
        }
    } else if app.phase == Phase::Upload {
        let inner = ul_block.inner(chunks[3]);
        frame.render_widget(ul_block, chunks[3]);
        let text = Paragraph::new(format!("  {:.1} Mbps", app.live_speed_mbps))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(text, inner);
    } else {
        let text = Paragraph::new("  —").block(ul_block).fg(Color::DarkGray);
        frame.render_widget(text, chunks[3]);
    }

    // High Scores
    let score_block = Block::default()
        .borders(Borders::ALL)
        .title(" High Scores (DL + UL) ");
    if app.scores.entries.is_empty() {
        let text = Paragraph::new("  No scores yet")
            .block(score_block)
            .fg(Color::DarkGray);
        frame.render_widget(text, chunks[4]);
    } else {
        let header = Row::new(vec!["#", "Combined", "Down", "Up", "Date"])
            .style(Style::default().fg(Color::White).bold());
        let rows: Vec<Row> = app
            .scores
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                Row::new(vec![
                    format!("{}", i + 1),
                    format!("{:.1}", e.combined_mbps),
                    format!("{:.1}", e.download_mbps),
                    format!("{:.1}", e.upload_mbps),
                    e.date.format("%Y-%m-%d %H:%M").to_string(),
                ])
            })
            .collect();
        let widths = [
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(18),
        ];
        let table = Table::new(rows, widths)
            .header(header)
            .block(score_block)
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(table, chunks[4]);
    }

    // Error
    if let Some(ref err) = app.error {
        let err_para = Paragraph::new(err.as_str())
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(err_para, chunks[5]);
    }

    // Footer
    let footer = Paragraph::new(" [Enter] Start  [q] Quit")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[6]);
}

fn setup_terminal() -> io::Result<Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = stdout().execute(LeaveAlternateScreen);
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Restore terminal on panic
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let mut terminal = setup_terminal()?;
    let mut app = App::new();
    let (tx, mut rx) = mpsc::channel::<SpeedMsg>(32);

    loop {
        terminal.draw(|frame| draw(frame, &app))?;

        // Drain all pending speed messages
        while let Ok(msg) = rx.try_recv() {
            match msg {
                SpeedMsg::Progress { current_mbps } => {
                    app.live_speed_mbps = current_mbps;
                }
                SpeedMsg::PhaseComplete { avg_mbps, samples } => match app.phase {
                    Phase::Download => {
                        app.download_result = Some(TestResult {
                            speed_mbps: avg_mbps,
                            samples,
                        });
                        app.live_speed_mbps = 0.0;
                        app.phase = Phase::Upload;
                    }
                    Phase::Upload => {
                        app.upload_result = Some(TestResult {
                            speed_mbps: avg_mbps,
                            samples,
                        });
                        app.live_speed_mbps = 0.0;
                        app.phase = Phase::Done;

                        if let Some(ref dl) = app.download_result {
                            app.scores.add(dl.speed_mbps, avg_mbps);
                        }
                    }
                    _ => {}
                },
                SpeedMsg::Error(e) => {
                    app.error = Some(e);
                    app.phase = Phase::Done;
                }
            }
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.should_quit = true;
                    }
                    KeyCode::Enter
                        if app.phase == Phase::Idle || app.phase == Phase::Done =>
                    {
                        app.phase = Phase::Download;
                        app.live_speed_mbps = 0.0;
                        app.download_result = None;
                        app.upload_result = None;
                        app.error = None;

                        let tx = tx.clone();
                        tokio::spawn(async move {
                            speedtest::run_download(tx.clone()).await;
                            speedtest::run_upload(tx).await;
                        });
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    restore_terminal();
    Ok(())
}
