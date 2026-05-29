// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};
use sprue_sdk::Client;
use sprue_sdk::types::*;
use std::io;
use std::num::NonZeroU32;
use std::time::Duration;
use tokio::time::Instant;

const PAGE_SIZE: u32 = 50;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const EVENT_POLL: Duration = Duration::from_millis(50);

/// What data needs loading after a key press.
enum Reload {
    None,
    Detail,
    Inspect,
    NextPage,
}

// ---------------------------------------------------------------------------
// Focus & navigation types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Services,
    Detail,
    ServerInspect,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DetailTab {
    Servers,
    Deployments,
}

impl DetailTab {
    const ALL: [DetailTab; 2] = [DetailTab::Servers, DetailTab::Deployments];
    fn title(self) -> &'static str {
        match self {
            DetailTab::Deployments => "Deployments",
            DetailTab::Servers => "Servers",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectTab {
    Blobs,
    Checkins,
}

impl InspectTab {
    const ALL: [InspectTab; 2] = [InspectTab::Blobs, InspectTab::Checkins];
    fn title(self) -> &'static str {
        match self {
            InspectTab::Blobs => "Backups",
            InspectTab::Checkins => "Checkins",
        }
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    client: Client,
    should_quit: bool,
    focus: Focus,

    // Services
    services: Vec<Service>,
    service_state: TableState,

    // Detail pane
    detail_tab: DetailTab,
    deployments: Vec<Deployment>,
    deployment_state: TableState,
    service_servers: Vec<ServerRegistration>,
    service_server_state: TableState,

    // Server inspect pane
    inspect_tab: InspectTab,
    inspect_server: Option<ServerRegistration>,
    blobs: Vec<Blob>,
    blob_state: TableState,
    blob_next_page: Option<String>,
    checkins: Vec<HealthCheck>,
    checkin_state: TableState,
    checkin_next_page: Option<String>,

    // Polling
    next_refresh: Instant,
    status: String,
}

impl App {
    fn new(client: Client) -> Self {
        Self {
            client,
            should_quit: false,
            focus: Focus::Services,
            services: Vec::new(),
            service_state: TableState::default(),
            detail_tab: DetailTab::Servers,
            deployments: Vec::new(),
            deployment_state: TableState::default(),
            service_servers: Vec::new(),
            service_server_state: TableState::default(),
            inspect_tab: InspectTab::Blobs,
            inspect_server: None,
            blobs: Vec::new(),
            blob_state: TableState::default(),
            blob_next_page: None,
            checkins: Vec::new(),
            checkin_state: TableState::default(),
            checkin_next_page: None,
            next_refresh: Instant::now(),
            status: "Loading...".into(),
        }
    }

    fn refresh_timer(&self) -> String {
        let remaining = self.next_refresh.saturating_duration_since(Instant::now());
        format!("{}s", remaining.as_secs())
    }

    // -- Data loading -------------------------------------------------------

    async fn refresh_all(&mut self) {
        self.load_services().await;
        self.load_detail().await;
        if self.inspect_server.is_some() {
            self.load_inspect().await;
        }
        self.next_refresh = Instant::now() + POLL_INTERVAL;
    }

    async fn load_services(&mut self) {
        match self.client.list_services().send().await {
            Ok(resp) => {
                self.services = resp.into_inner();
                if !self.services.is_empty() && self.service_state.selected().is_none() {
                    self.service_state.select(Some(0));
                }
                self.status = format!("{} service(s)", self.services.len());
            }
            Err(err) => self.status = format!("Error: {}", err),
        }
    }

    async fn load_detail(&mut self) {
        let sid = match self.selected_service() {
            Some(s) => s.id.clone(),
            None => return,
        };

        // Always load both so switching tabs is instant.
        match self
            .client
            .list_deployments()
            .service(sid.to_string())
            .send()
            .await
        {
            Ok(r) => {
                self.deployments = r.into_inner();
                if !self.deployments.is_empty() && self.deployment_state.selected().is_none() {
                    self.deployment_state.select(Some(0));
                }
            }
            Err(e) => self.status = format!("Deployments: {}", e),
        }

        match self
            .client
            .get_service_servers()
            .service(sid.to_string())
            .send()
            .await
        {
            Ok(r) => {
                self.service_servers = r.into_inner();
                if !self.service_servers.is_empty()
                    && self.service_server_state.selected().is_none()
                {
                    self.service_server_state.select(Some(0));
                }
            }
            Err(e) => self.status = format!("Servers: {}", e),
        }
    }

    async fn load_inspect(&mut self) {
        self.blobs.clear();
        self.blob_state.select(None);
        self.blob_next_page = None;
        self.checkins.clear();
        self.checkin_state.select(None);
        self.checkin_next_page = None;

        self.load_blobs_page(None).await;
        self.load_checkins_page(None).await;
    }

    async fn load_blobs_page(&mut self, page_token: Option<String>) {
        let server = match &self.inspect_server {
            Some(s) => s.clone(),
            None => return,
        };

        let mut req = self
            .client
            .list_server_blobs()
            .server(server.id.0)
            .limit(NonZeroU32::new(PAGE_SIZE).unwrap());
        if let Some(ref token) = page_token {
            req = req.page_token(token.clone());
        }

        match req.send().await {
            Ok(r) => {
                let page = r.into_inner();
                self.blobs.extend(page.items);
                self.blob_next_page = page.next_page;
                if !self.blobs.is_empty() && self.blob_state.selected().is_none() {
                    self.blob_state.select(Some(0));
                }
            }
            Err(e) => self.status = format!("Blobs: {}", e),
        }
    }

    async fn load_checkins_page(&mut self, page_token: Option<String>) {
        let server = match &self.inspect_server {
            Some(s) => s.clone(),
            None => return,
        };

        let mut req = self
            .client
            .list_server_checkins()
            .server(server.id.0)
            .limit(NonZeroU32::new(PAGE_SIZE).unwrap());
        if let Some(ref token) = page_token {
            req = req.page_token(token.clone());
        }

        match req.send().await {
            Ok(r) => {
                let page = r.into_inner();
                self.checkins.extend(page.items);
                self.checkin_next_page = page.next_page;
                if !self.checkins.is_empty() && self.checkin_state.selected().is_none() {
                    self.checkin_state.select(Some(0));
                }
            }
            Err(e) => self.status = format!("Checkins: {}", e),
        }
    }

    fn selected_service(&self) -> Option<&Service> {
        self.service_state
            .selected()
            .and_then(|i| self.services.get(i))
    }

    fn selected_detail_server(&self) -> Option<&ServerRegistration> {
        self.service_server_state
            .selected()
            .and_then(|i| self.service_servers.get(i))
    }

    // -- Input handling -----------------------------------------------------

    /// Returns what data needs loading after this key press.
    fn handle_key(&mut self, key: KeyCode) -> Reload {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                match self.focus {
                    Focus::ServerInspect => {
                        self.focus = Focus::Detail;
                        self.inspect_server = None;
                    }
                    Focus::Detail => self.focus = Focus::Services,
                    Focus::Services => self.should_quit = true,
                }
                return Reload::None;
            }
            KeyCode::Enter => {
                match self.focus {
                    Focus::Services => {
                        self.focus = Focus::Detail;
                        return Reload::Detail;
                    }
                    Focus::Detail if self.detail_tab == DetailTab::Servers => {
                        if let Some(s) = self.selected_detail_server() {
                            self.inspect_server = Some(s.clone());
                            self.focus = Focus::ServerInspect;
                            return Reload::Inspect;
                        }
                    }
                    _ => {}
                }
                return Reload::None;
            }
            _ => {}
        }

        match self.focus {
            Focus::Services => self.handle_services_key(key),
            Focus::Detail => self.handle_detail_key(key),
            Focus::ServerInspect => self.handle_inspect_key(key),
        }
    }

    fn handle_services_key(&mut self, key: KeyCode) -> Reload {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                nav_up(&mut self.service_state, self.services.len());
                self.clear_detail();
                Reload::Detail
            }
            KeyCode::Down | KeyCode::Char('j') => {
                nav_down(&mut self.service_state, self.services.len());
                self.clear_detail();
                Reload::Detail
            }
            _ => Reload::None,
        }
    }

    fn handle_detail_key(&mut self, key: KeyCode) -> Reload {
        match key {
            KeyCode::Left => {
                self.detail_tab = DetailTab::Servers;
                Reload::None
            }
            KeyCode::Right => {
                self.detail_tab = DetailTab::Deployments;
                Reload::None
            }
            KeyCode::Up => {
                match self.detail_tab {
                    DetailTab::Deployments => {
                        nav_up(&mut self.deployment_state, self.deployments.len())
                    }
                    DetailTab::Servers => {
                        nav_up(&mut self.service_server_state, self.service_servers.len())
                    }
                }
                Reload::None
            }
            KeyCode::Down => {
                match self.detail_tab {
                    DetailTab::Deployments => {
                        nav_down(&mut self.deployment_state, self.deployments.len())
                    }
                    DetailTab::Servers => {
                        nav_down(&mut self.service_server_state, self.service_servers.len())
                    }
                }
                Reload::None
            }
            _ => Reload::None,
        }
    }

    fn handle_inspect_key(&mut self, key: KeyCode) -> Reload {
        match key {
            KeyCode::Left | KeyCode::Char('h') => self.inspect_tab = InspectTab::Blobs,
            KeyCode::Right | KeyCode::Char('l') => self.inspect_tab = InspectTab::Checkins,
            KeyCode::Up | KeyCode::Char('k') => match self.inspect_tab {
                InspectTab::Blobs => nav_up(&mut self.blob_state, self.blobs.len()),
                InspectTab::Checkins => nav_up(&mut self.checkin_state, self.checkins.len()),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.inspect_tab {
                InspectTab::Blobs => {
                    nav_down(&mut self.blob_state, self.blobs.len());
                    if self.blob_next_page.is_some() {
                        if let Some(i) = self.blob_state.selected() {
                            if i + 5 >= self.blobs.len() {
                                return Reload::NextPage;
                            }
                        }
                    }
                }
                InspectTab::Checkins => {
                    nav_down(&mut self.checkin_state, self.checkins.len());
                    if self.checkin_next_page.is_some() {
                        if let Some(i) = self.checkin_state.selected() {
                            if i + 5 >= self.checkins.len() {
                                return Reload::NextPage;
                            }
                        }
                    }
                }
            },
            _ => {}
        }
        Reload::None
    }

    fn clear_detail(&mut self) {
        self.deployments.clear();
        self.service_servers.clear();
        self.deployment_state.select(None);
        self.service_server_state.select(None);
        self.inspect_server = None;
    }

    // -- Drawing ------------------------------------------------------------

    fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // title bar
                Constraint::Min(0),    // body
                Constraint::Length(1), // status bar
            ])
            .split(frame.area());

        self.draw_title(frame, chunks[0]);

        if self.inspect_server.is_some() {
            self.draw_inspect(frame, chunks[1]);
        } else {
            self.draw_main(frame, chunks[1]);
        }

        self.draw_status(frame, chunks[2]);
    }

    fn draw_title(&self, frame: &mut Frame, area: Rect) {
        let breadcrumb = match self.focus {
            Focus::Services => " Sprue › Services".to_string(),
            Focus::Detail => {
                let name = self
                    .selected_service()
                    .map(|s| s.name.as_str())
                    .unwrap_or("?");
                format!(" Sprue › {} › {}", name, self.detail_tab.title())
            }
            Focus::ServerInspect => {
                let name = self
                    .selected_service()
                    .map(|s| s.name.as_str())
                    .unwrap_or("?");
                let sid = self
                    .inspect_server
                    .as_ref()
                    .map(|s| short_uuid(&s.id.0))
                    .unwrap_or_else(|| "?".into());
                format!(
                    " Sprue › {} › Server {} › {}",
                    name,
                    sid,
                    self.inspect_tab.title()
                )
            }
        };

        let timer = self.refresh_timer();
        let right = format!("⟳ {} ", timer);

        let available = area.width as usize;
        let pad = available.saturating_sub(breadcrumb.len() + right.len());

        let line = Line::from(vec![
            Span::styled(
                breadcrumb,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(pad)),
            Span::styled(right, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let help = match self.focus {
            Focus::Services => " ↑↓: navigate | Enter: details | r: refresh | q: quit ",
            Focus::Detail => {
                " ↑↓: navigate | ←→: tab | Enter: inspect server | Esc: back | r: refresh | q: quit "
            }
            Focus::ServerInspect => {
                " ↑↓: navigate (auto-loads more) | ←→: tab | Esc: back | r: refresh "
            }
        };
        let line = Line::from(vec![
            Span::styled(help, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(&self.status, Style::default().fg(Color::Yellow)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn draw_main(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area);

        self.draw_services(frame, chunks[0]);
        self.draw_detail(frame, chunks[1]);
    }

    fn draw_services(&mut self, frame: &mut Frame, area: Rect) {
        let highlight = if self.focus == Focus::Services {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };

        let rows: Vec<Row> = self
            .services
            .iter()
            .map(|s| {
                Row::new(vec![
                    Cell::from(s.name.clone()),
                    Cell::from(short_uuid(&s.id.0)),
                ])
            })
            .collect();

        let table = Table::new(rows, [Constraint::Min(16), Constraint::Length(10)])
            .header(
                Row::new(vec!["Name", "ID"]).style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(Block::default().borders(Borders::ALL).title(" Services "))
            .row_highlight_style(highlight);

        frame.render_stateful_widget(table, area, &mut self.service_state);
    }

    fn draw_detail(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Sub-tabs
        let titles: Vec<Line> = DetailTab::ALL
            .iter()
            .map(|t| Line::from(t.title()))
            .collect();
        let selected = DetailTab::ALL
            .iter()
            .position(|t| *t == self.detail_tab)
            .unwrap_or(0);
        let tabs = Tabs::new(titles)
            .select(selected)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, chunks[0]);

        match self.detail_tab {
            DetailTab::Deployments => self.draw_deployments(frame, chunks[1]),
            DetailTab::Servers => self.draw_servers(frame, chunks[1]),
        }
    }

    fn draw_deployments(&mut self, frame: &mut Frame, area: Rect) {
        let highlight = detail_highlight(self.focus == Focus::Detail);

        let rows: Vec<Row> = self
            .deployments
            .iter()
            .map(|d| {
                Row::new(vec![
                    Cell::from(short_uuid(&d.id.0)),
                    Cell::from(short_uuid(&d.project_id.0)),
                    Cell::from(short_uuid(&d.silo_id.0)),
                    Cell::from(d.created_at.format("%Y-%m-%d %H:%M").to_string()),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(18),
            ],
        )
        .header(
            Row::new(vec!["ID", "Project", "Silo", "Created"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Deployments "),
        )
        .row_highlight_style(highlight);

        frame.render_stateful_widget(table, area, &mut self.deployment_state);
    }

    fn draw_servers(&mut self, frame: &mut Frame, area: Rect) {
        let highlight = detail_highlight(self.focus == Focus::Detail);

        let rows: Vec<Row> = self.service_servers.iter().map(|s| server_row(s)).collect();

        let table = server_table(rows)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Servers (Enter to inspect) "),
            )
            .row_highlight_style(highlight);

        frame.render_stateful_widget(table, area, &mut self.service_server_state);
    }

    fn draw_inspect(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // server summary
                Constraint::Length(3), // inspect tabs
                Constraint::Min(0),    // inspect body
            ])
            .split(area);

        // Server summary
        if let Some(ref server) = self.inspect_server {
            let state_style = state_color(server.state);
            let summary = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(" Instance: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(server.instance_id.0.to_string()),
                    Span::raw("  "),
                    Span::styled(" State: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{:?}", server.state), state_style),
                ]),
                Line::from(vec![
                    Span::styled(" Project:  ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(server.project_id.0.to_string()),
                    Span::raw("  "),
                    Span::styled(" Silo: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(server.silo_id.0.to_string()),
                ]),
            ])
            .block(Block::default().borders(Borders::ALL).title(" Server "));
            frame.render_widget(summary, chunks[0]);
        }

        // Inspect sub-tabs
        let titles: Vec<Line> = InspectTab::ALL
            .iter()
            .map(|t| Line::from(t.title()))
            .collect();
        let selected = InspectTab::ALL
            .iter()
            .position(|t| *t == self.inspect_tab)
            .unwrap_or(0);
        let tabs = Tabs::new(titles)
            .select(selected)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, chunks[1]);

        match self.inspect_tab {
            InspectTab::Blobs => self.draw_blobs(frame, chunks[2]),
            InspectTab::Checkins => self.draw_checkins(frame, chunks[2]),
        }
    }

    fn draw_blobs(&mut self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self
            .blobs
            .iter()
            .map(|b| {
                let state_str = format!("{:?}", b.state);
                Row::new(vec![
                    Cell::from(short_uuid(&b.id.0)),
                    Cell::from(state_str),
                    Cell::from(format_size(b.size)),
                    Cell::from(format_size(b.total_size)),
                    Cell::from(b.blob_time.format("%Y-%m-%d %H:%M").to_string()),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(24),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(18),
            ],
        )
        .header(
            Row::new(vec!["ID", "State", "Size", "Total", "Time"])
                .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Backups ({}) ", self.blobs.len())),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

        frame.render_stateful_widget(table, area, &mut self.blob_state);
    }

    fn draw_checkins(&mut self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self
            .checkins
            .iter()
            .map(|c| {
                Row::new(vec![
                    Cell::from(short_uuid(&c.id.0)),
                    Cell::from(c.checked_in_at.format("%Y-%m-%d %H:%M:%S").to_string()),
                ])
            })
            .collect();

        let latest = self
            .checkins
            .first()
            .map(|c| {
                let ago = chrono::Utc::now() - c.checked_in_at;
                format!(" Latest: {}s ago ", ago.num_seconds())
            })
            .unwrap_or_default();

        let table = Table::new(rows, [Constraint::Length(10), Constraint::Min(22)])
            .header(
                Row::new(vec!["ID", "Checked In"])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(Block::default().borders(Borders::ALL).title(format!(
                " Checkins ({}) {}",
                self.checkins.len(),
                latest
            )))
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_stateful_widget(table, area, &mut self.checkin_state);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn nav_up(state: &mut TableState, len: usize) {
    if len == 0 {
        return;
    }
    let i = state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
    state.select(Some(i));
}

fn nav_down(state: &mut TableState, len: usize) {
    if len == 0 {
        return;
    }
    let max = len.saturating_sub(1);
    let i = state.selected().map(|i| (i + 1).min(max)).unwrap_or(0);
    state.select(Some(i));
}

fn detail_highlight(focused: bool) -> Style {
    if focused {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

fn state_color(state: ServerRegistrationState) -> Style {
    match state {
        ServerRegistrationState::Accepted => Style::default().fg(Color::Green),
        ServerRegistrationState::Pending | ServerRegistrationState::Proven => {
            Style::default().fg(Color::Yellow)
        }
        ServerRegistrationState::Rejected | ServerRegistrationState::Terminated => {
            Style::default().fg(Color::Red)
        }
        ServerRegistrationState::Expired => Style::default().fg(Color::DarkGray),
    }
}

fn server_row(s: &ServerRegistration) -> Row<'static> {
    Row::new(vec![
        Cell::from(short_uuid(&s.id.0)),
        Cell::from(short_uuid(&s.instance_id.0)),
        Cell::from(Span::styled(format!("{:?}", s.state), state_color(s.state))),
        Cell::from(short_uuid(&s.project_id.0)),
        Cell::from(short_uuid(&s.silo_id.0)),
        Cell::from(s.updated_at.format("%Y-%m-%d %H:%M").to_string()),
    ])
}

fn server_table(rows: Vec<Row<'_>>) -> Table<'_> {
    Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(18),
        ],
    )
    .header(
        Row::new(vec![
            "ID", "Instance", "State", "Project", "Silo", "Updated",
        ])
        .style(Style::default().add_modifier(Modifier::BOLD)),
    )
}

fn short_uuid(uuid: &uuid::Uuid) -> String {
    uuid.to_string()[..8].to_string()
}

fn format_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run(client: Client) -> io::Result<()> {
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)?;

    let terminal = ratatui::init();
    let result = run_app(terminal, client).await;

    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
    ratatui::restore();

    result
}

async fn run_app(mut terminal: DefaultTerminal, client: Client) -> io::Result<()> {
    let mut app = App::new(client);
    app.refresh_all().await;
    let mut last_service_idx = app.service_state.selected();

    loop {
        terminal.draw(|f| app.draw(f))?;

        // Auto-refresh
        if Instant::now() >= app.next_refresh {
            app.refresh_all().await;
            last_service_idx = app.service_state.selected();
            continue;
        }

        if event::poll(EVENT_POLL)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.code == KeyCode::Char('r') {
                    app.status = "Refreshing...".into();
                    app.refresh_all().await;
                    last_service_idx = app.service_state.selected();
                    continue;
                }

                let needs_reload = app.handle_key(key.code);

                if app.should_quit {
                    return Ok(());
                }

                match needs_reload {
                    Reload::None => {}
                    Reload::Detail => {
                        app.load_detail().await;
                        last_service_idx = app.service_state.selected();
                    }
                    Reload::Inspect => {
                        app.load_inspect().await;
                    }
                    Reload::NextPage => match app.inspect_tab {
                        InspectTab::Blobs => {
                            if let Some(token) = app.blob_next_page.clone() {
                                app.load_blobs_page(Some(token)).await;
                            }
                        }
                        InspectTab::Checkins => {
                            if let Some(token) = app.checkin_next_page.clone() {
                                app.load_checkins_page(Some(token)).await;
                            }
                        }
                    },
                }
            }
        }

        // Detect service selection change (from navigation)
        if app.focus == Focus::Services {
            let current = app.service_state.selected();
            if current != last_service_idx {
                last_service_idx = current;
                app.clear_detail();
                app.load_detail().await;
            }
        }
    }
}
