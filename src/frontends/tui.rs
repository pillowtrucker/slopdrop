//! TUI frontend for slopdrop using ratatui
//!
//! Provides a full-screen terminal interface with multiple panes

use crate::config::{SecurityConfig, TclConfig};
use crate::frontend::Frontend;
use crate::tcl_service::{EvalContext, TclService};
use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, RwLock};
use tracing::info;

/// TUI frontend configuration
#[derive(Clone, Debug)]
pub struct TuiConfig {
    /// Username for evaluation context
    pub username: String,
    /// Whether the user has admin privileges
    pub is_admin: bool,
    /// Refresh rate in milliseconds
    pub refresh_rate_ms: u64,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            username: whoami::username(),
            is_admin: true,
            refresh_rate_ms: 100,
        }
    }
}

/// Application state for TUI
struct AppState {
    /// Lines of output to display
    output_lines: Vec<String>,
    /// Current input buffer
    input: String,
    /// Cursor position in input
    cursor_pos: usize,
    /// Git history
    history: Vec<String>,
    /// Whether more output is available
    more_available: bool,
    /// Status message
    status: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            output_lines: vec!["Welcome to Slopdrop TUI".to_string()],
            input: String::new(),
            cursor_pos: 0,
            history: vec![],
            more_available: false,
            status: "Ready".to_string(),
        }
    }
}

/// TUI frontend implementation
pub struct TuiFrontend {
    name: String,
    config: TuiConfig,
    tcl_service: TclService,
    running: bool,
}

impl TuiFrontend {
    /// Create a new TUI frontend
    pub fn new(
        config: TuiConfig,
        security_config: SecurityConfig,
        tcl_config: TclConfig,
    ) -> Result<Self> {
        let channel_members = Arc::new(RwLock::new(HashMap::new()));
        let tcl_service = TclService::new(security_config, tcl_config, channel_members)?;

        Ok(Self {
            name: "TUI".to_string(),
            config,
            tcl_service,
            running: false,
        })
    }

    /// Run the TUI application
    async fn run_app(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        let mut state = AppState::default();

        // Load initial git history
        self.update_history(&mut state).await;

        // Main loop
        while self.running {
            // Draw UI
            terminal
                .draw(|f| self.draw_ui(f, &state))
                .context("Failed to draw UI")?;

            // Handle events with timeout
            if event::poll(std::time::Duration::from_millis(self.config.refresh_rate_ms))
                .context("Failed to poll events")?
            {
                if let Event::Key(key) = event::read().context("Failed to read event")? {
                    self.handle_key_event(key, &mut state).await?;
                }
            }
        }

        // Restore terminal
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .context("Failed to leave alternate screen")?;
        terminal.show_cursor().context("Failed to show cursor")?;

        Ok(())
    }

    /// Draw the UI
    fn draw_ui(&self, f: &mut Frame, state: &AppState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),      // Output area
                Constraint::Length(3),    // Input area
                Constraint::Length(10),   // History area
                Constraint::Length(3),    // Status area
            ])
            .split(f.size());

        // Output area
        let output_text: Vec<Line> = state
            .output_lines
            .iter()
            .map(|line| Line::from(line.clone()))
            .collect();

        let output = Paragraph::new(output_text)
            .block(
                Block::default()
                    .title("Output")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(output, chunks[0]);

        // Input area
        let input_text = if state.more_available {
            format!("{} (more available - press F2)", state.input)
        } else {
            state.input.clone()
        };

        let input = Paragraph::new(input_text)
            .block(
                Block::default()
                    .title("Input (Ctrl+Enter to eval, Ctrl+C to quit)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            );
        f.render_widget(input, chunks[1]);

        // History area
        let history_items: Vec<ListItem> = state
            .history
            .iter()
            .map(|h| ListItem::new(h.clone()))
            .collect();

        let history = List::new(history_items).block(
            Block::default()
                .title("Git History (F3 to refresh)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        f.render_widget(history, chunks[2]);

        // Status area
        let status = Paragraph::new(state.status.clone()).block(
            Block::default()
                .title("Status")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        );
        f.render_widget(status, chunks[3]);
    }

    /// Handle keyboard events
    async fn handle_key_event(
        &mut self,
        key: event::KeyEvent,
        state: &mut AppState,
    ) -> Result<()> {
        match (key.code, key.modifiers) {
            // Ctrl+C: Quit
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.running = false;
            }
            // Ctrl+Enter: Evaluate
            (KeyCode::Enter, KeyModifiers::CONTROL) => {
                self.evaluate_input(state).await?;
            }
            // F2: More
            (KeyCode::F(2), _) => {
                self.get_more(state).await?;
            }
            // F3: Refresh history
            (KeyCode::F(3), _) => {
                self.update_history(state).await;
            }
            // Backspace
            (KeyCode::Backspace, _) => {
                if state.cursor_pos > 0 {
                    state.input.remove(state.cursor_pos - 1);
                    state.cursor_pos -= 1;
                }
            }
            // Regular character input
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                state.input.insert(state.cursor_pos, c);
                state.cursor_pos += 1;
            }
            // Enter: newline in input
            (KeyCode::Enter, KeyModifiers::NONE) => {
                state.input.insert(state.cursor_pos, '\n');
                state.cursor_pos += 1;
            }
            // Left arrow
            (KeyCode::Left, _) => {
                if state.cursor_pos > 0 {
                    state.cursor_pos -= 1;
                }
            }
            // Right arrow
            (KeyCode::Right, _) => {
                if state.cursor_pos < state.input.len() {
                    state.cursor_pos += 1;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Evaluate the current input
    async fn evaluate_input(&mut self, state: &mut AppState) -> Result<()> {
        if state.input.trim().is_empty() {
            return Ok(());
        }

        let code = state.input.clone();
        state.status = format!("Evaluating: {}", code.lines().next().unwrap_or(""));

        let ctx = EvalContext::new(self.config.username.clone(), "local".to_string())
            .with_admin(self.config.is_admin);

        match self.tcl_service.eval(&code, ctx).await {
            Ok(response) => {
                // Add input to output
                state.output_lines.push(format!("> {}", code));

                // Add result to output
                for line in &response.output {
                    state.output_lines.push(line.clone());
                }

                state.more_available = response.more_available;

                if let Some(commit_info) = response.commit_info {
                    state.output_lines.push(format!(
                        "[Git] {} | {} files (+{} -{})",
                        &commit_info.commit_id[..8],
                        commit_info.files_changed,
                        commit_info.insertions,
                        commit_info.deletions
                    ));

                    // Refresh history
                    self.update_history(state).await;
                }

                state.status = "Evaluation complete".to_string();
            }
            Err(e) => {
                state.output_lines.push(format!("Error: {}", e));
                state.status = format!("Error: {}", e);
            }
        }

        // Clear input
        state.input.clear();
        state.cursor_pos = 0;

        // Keep output scrolled to bottom by limiting lines
        if state.output_lines.len() > 1000 {
            state.output_lines.drain(0..500);
        }

        Ok(())
    }

    /// Get more paginated output
    async fn get_more(&mut self, state: &mut AppState) -> Result<()> {
        let ctx = EvalContext::new(self.config.username.clone(), "local".to_string())
            .with_admin(self.config.is_admin);

        match self.tcl_service.more(ctx).await {
            Ok(response) => {
                for line in &response.output {
                    state.output_lines.push(line.clone());
                }
                state.more_available = response.more_available;
                state.status = "Retrieved more output".to_string();
            }
            Err(e) => {
                state.status = format!("Error getting more: {}", e);
            }
        }

        Ok(())
    }

    /// Update git history display
    async fn update_history(&mut self, state: &mut AppState) {
        match self.tcl_service.history(10).await {
            Ok(commits) => {
                state.history = commits
                    .into_iter()
                    .map(|c| {
                        format!(
                            "{} - {} - {}",
                            &c.commit_id[..8],
                            c.author,
                            c.message.lines().next().unwrap_or("")
                        )
                    })
                    .collect();
            }
            Err(e) => {
                state.status = format!("Failed to get history: {}", e);
            }
        }
    }
}

#[async_trait]
impl Frontend for TuiFrontend {
    fn name(&self) -> &str {
        &self.name
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting TUI frontend");
        self.running = true;
        self.run_app().await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping TUI frontend");
        self.running = false;
        self.tcl_service.shutdown();
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running
    }
}
