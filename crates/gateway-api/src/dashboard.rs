//! Terminal UI Dashboard for the AI Gateway.
//!
//! Run with `gateway dashboard` to open a real-time monitoring view.

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Sparkline, Table, Tabs, Wrap},
    Frame, Terminal,
};
use tokio::sync::mpsc;

/// Dashboard application state.
struct DashboardApp {
    /// Last fetch time
    last_update: Instant,
    /// Prometheus metrics text
    metrics_text: String,
    /// Health status text
    health_text: String,
    /// Active profile name
    profile_name: String,
    /// Whether to quit
    should_quit: bool,
    /// Selected tab
    selected_tab: usize,
    /// Error message if fetch fails
    error_message: Option<String>,
    /// History for sparklines (last 60 values)
    request_history: Vec<u64>,
    latency_history: Vec<u64>,
}

impl DashboardApp {
    fn new(profile_name: String) -> Self {
        Self {
            last_update: Instant::now() - Duration::from_secs(10),
            metrics_text: "Loading metrics...".to_string(),
            health_text: "Loading health...".to_string(),
            profile_name,
            should_quit: false,
            selected_tab: 0,
            error_message: None,
            request_history: vec![0; 60],
            latency_history: vec![0; 60],
        }
    }

    fn on_tick(&mut self) {
        // Triggered by the tick interval
    }

    fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('1') => self.selected_tab = 0,
            KeyCode::Char('2') => self.selected_tab = 1,
            KeyCode::Char('3') => self.selected_tab = 2,
            KeyCode::Right => self.selected_tab = (self.selected_tab + 1) % 3,
            KeyCode::Left => {
                self.selected_tab = if self.selected_tab == 0 { 2 } else { self.selected_tab - 1 };
            }
            _ => {}
        }
    }
}

/// Run the dashboard TUI.
pub async fn run(profile_name: String) -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = DashboardApp::new(profile_name);

    // Channel for async data fetching
    let (tx, mut rx) = mpsc::channel::<DashboardData>(10);

    // Spawn data fetcher task
    let fetcher = tokio::spawn(data_fetcher(tx));

    // Main loop
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        // Poll for new data
        while let Ok(data) = rx.try_recv() {
            app.metrics_text = data.metrics;
            app.health_text = data.health;
            app.error_message = data.error;
            app.last_update = Instant::now();
            // Update history
            app.request_history.push(data.request_count);
            if app.request_history.len() > 60 {
                app.request_history.remove(0);
            }
            app.latency_history.push(data.avg_latency_ms);
            if app.latency_history.len() > 60 {
                app.latency_history.remove(0);
            }
        }

        // Draw UI
        terminal.draw(|f| draw_ui(f, &app))?;

        // Handle input with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key.code);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    fetcher.abort();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// Data fetched from the gateway.
struct DashboardData {
    metrics: String,
    health: String,
    error: Option<String>,
    request_count: u64,
    avg_latency_ms: u64,
}

/// Background task that fetches metrics and health from the gateway.
async fn data_fetcher(tx: mpsc::Sender<DashboardData>) {
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(Duration::from_secs(2));

    loop {
        interval.tick().await;

        let metrics = client
            .get("http://localhost:8080/metrics")
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        let health = client
            .get("http://localhost:8080/health")
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        let (metrics_text, health_text, error) = match (metrics, health) {
            (Ok(m), Ok(h)) => {
                let mt = m.text().await.unwrap_or_else(|_| "(parse error)".into());
                let ht = h.text().await.unwrap_or_else(|_| "(parse error)".into());
                (mt, ht, None)
            }
            (Err(e), _) => (String::new(), String::new(), Some(format!("Metrics fetch failed: {}", e))),
            (_, Err(e)) => (String::new(), String::new(), Some(format!("Health fetch failed: {}", e))),
        };

        // Parse simple metrics
        let request_count = parse_counter(&metrics_text, "gateway_request_total");
        let avg_latency_ms = parse_avg_histogram(&metrics_text, "gateway_request_duration_ms");

        let _ = tx.send(DashboardData {
            metrics: metrics_text,
            health: health_text,
            error,
            request_count,
            avg_latency_ms,
        }).await;
    }
}

/// Parse a counter value from Prometheus text.
fn parse_counter(text: &str, name: &str) -> u64 {
    for line in text.lines() {
        if line.starts_with(name) {
            if let Some(val) = line.rsplit(' ').next() {
                return val.parse().unwrap_or(0);
            }
        }
    }
    0
}

/// Parse average histogram value (very rough approximation).
fn parse_avg_histogram(_text: &str, _name: &str) -> u64 {
    // For simplicity, return 0; a real implementation would parse bucket sums/counts
    0
}

// ── UI Rendering ─────────────────────────────────────────────────────────────

fn draw_ui(f: &mut Frame, app: &DashboardApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_tabs_content(f, app, chunks[1]);
    draw_footer(f, chunks[2]);

    // Draw error popup if any
    if let Some(ref err) = app.error_message {
        draw_error_popup(f, err);
    }
}

fn draw_header(f: &mut Frame, app: &DashboardApp, area: Rect) {
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("AI Gateway ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("| Profile: "),
            Span::styled(&app.profile_name, Style::default().fg(Color::Green)),
            Span::raw(" | "),
            Span::raw(format!("Last update: {:?} ago", app.last_update.elapsed())),
        ]),
    ])
    .block(Block::default().borders(Borders::BOTTOM).border_style(Color::Cyan))
    .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn draw_tabs_content(f: &mut Frame, app: &DashboardApp, area: Rect) {
    let titles = vec!["Overview", "Providers", "Metrics"];
    let tabs = Tabs::new(titles)
        .select(app.selected_tab)
        .block(Block::default().borders(Borders::ALL).title("Tabs"))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(Span::raw(" | "));

    let inner = area.inner(Margin { horizontal: 1, vertical: 1 });
    let tab_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    f.render_widget(tabs, tab_chunks[0]);

    match app.selected_tab {
        0 => draw_overview(f, app, tab_chunks[1]),
        1 => draw_providers(f, app, tab_chunks[1]),
        2 => draw_metrics_raw(f, app, tab_chunks[1]),
        _ => {}
    }
}

fn draw_overview(f: &mut Frame, app: &DashboardApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Request sparkline
    let sparkline = Sparkline::default()
        .data(&app.request_history)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title("Requests/sec (last 60s)")
                .borders(Borders::ALL),
        );
    f.render_widget(sparkline, chunks[0]);

    // Right: Health status
    let health_color = if app.health_text.contains("healthy") || app.health_text.contains("ok") {
        Color::Green
    } else {
        Color::Red
    };

    let health = Paragraph::new(app.health_text.clone())
        .block(
            Block::default()
                .title("Health Status")
                .borders(Borders::ALL)
                .border_style(health_color),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(health, chunks[1]);
}

fn draw_providers(f: &mut Frame, app: &DashboardApp, area: Rect) {
    // Parse provider health from metrics
    let mut rows = vec![];
    for line in app.metrics_text.lines() {
        if line.starts_with("gateway_provider_health") {
            let provider = extract_label(line, "provider").unwrap_or_else(|| "unknown".into());
            let org = extract_label(line, "org").unwrap_or_else(|| "unknown".into());
            let value = line.rsplit(' ').next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(-1.0);
            let status = if value >= 0.5 { "✓ Healthy" } else { "✗ Unhealthy" };
            let color = if value >= 0.5 { Color::Green } else { Color::Red };

            rows.push(Row::new(vec![
                Cell::from(provider),
                Cell::from(org),
                Cell::from(Span::styled(status, Style::default().fg(color))),
                Cell::from(format!("{:.0}", value)),
            ]));
        }
    }

    if rows.is_empty() {
        let msg = Paragraph::new("No provider health data available yet.\n\nProviders will appear here once the health check worker runs.")
            .block(Block::default().title("Providers").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(25),
            Constraint::Percentage(15),
        ],
    )
    .header(
        Row::new(vec!["Provider", "Org", "Status", "Value"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().title("Provider Health").borders(Borders::ALL))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(table, area);
}

fn draw_metrics_raw(f: &mut Frame, app: &DashboardApp, area: Rect) {
    let metrics = Paragraph::new(app.metrics_text.clone())
        .block(Block::default().title("Prometheus Metrics").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(metrics, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new("q:Quit | 1:Overview | 2:Providers | 3:Metrics | ←/→:Switch tab")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(footer, area);
}

fn draw_error_popup(f: &mut Frame, error: &str) {
    let block = Block::default()
        .title("Error")
        .borders(Borders::ALL)
        .border_style(Color::Red)
        .style(Style::default().bg(Color::Black));

    let area = centered_rect(60, 20, f.area());
    let paragraph = Paragraph::new(error)
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn extract_label(line: &str, key: &str) -> Option<String> {
    let search = format!("{}=\"", key);
    if let Some(start) = line.find(&search) {
        let rest = &line[start + search.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}
