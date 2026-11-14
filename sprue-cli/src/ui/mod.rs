// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The `crossterm` re-exported by `ratatui` must be used rather than a direct
// dependency on the `crossterm` crate. Crossterm records the terminal mode it
// found on entry in a process global, so two versions of it in the same binary
// each keep their own snapshot and clobber one another on teardown, leaving the
// terminal in raw mode after exit.
use chrono::{DateTime, Utc};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Padding, Paragraph, Row, Table, TableState, Tabs},
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

// Oxide brand colours, taken from the design tokens the docs site ships at
// <https://docs.oxide.computer>. Those are published in OKLCH; the sRGB
// conversions are inlined here, and the token name is kept so the source of
// each value stays traceable.

/// `yellow-800` — the single accent, used for every "this is the active thing"
/// cue: the breadcrumb, the selected tab, and the focused pane's border.
const ACCENT: Color = Color::Rgb(0xFE, 0xBB, 0x55);

/// `green-800` — the marker dots down the left of every list.
const MARK: Color = Color::Rgb(0x00, 0xD8, 0x92);

/// `neutral-600` — secondary text that should sit behind the data without
/// disappearing: column headers, the key hints, the refresh countdown.
const MUTED: Color = Color::Rgb(0x81, 0x82, 0x84);

/// Width of the marker column. Three columns so the dot sits centred with a
/// space either side, and fixed so a terminal that renders the dots
/// double-width cannot shift the columns beside them.
const MARK_COLUMN: Constraint = Constraint::Length(3);

/// What data needs loading after a key press.
enum Reload {
    None,
    Detail,
    Inspect,
    NextPage,
    /// Run the action the user just confirmed.
    Action,
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
// Server registration actions
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum ServerAction {
    Accept,
    Terminate,
    Delete,
}

impl ServerAction {
    fn label(self) -> &'static str {
        match self {
            ServerAction::Accept => "Accept",
            ServerAction::Terminate => "Terminate",
            ServerAction::Delete => "Delete",
        }
    }

    fn progressive(self) -> &'static str {
        match self {
            ServerAction::Accept => "Accepting",
            ServerAction::Terminate => "Terminating",
            ServerAction::Delete => "Deleting",
        }
    }

    fn past(self) -> &'static str {
        match self {
            ServerAction::Accept => "Accepted",
            ServerAction::Terminate => "Terminated",
            ServerAction::Delete => "Deleted",
        }
    }

    fn description(self) -> &'static str {
        match self {
            ServerAction::Accept => "Add the server as a representative instance of the service.",
            ServerAction::Terminate => {
                "Remove the server from the pool of representative instances."
            }
            ServerAction::Delete => "Permanently delete the registration. This cannot be undone.",
        }
    }

    /// Whether the registration state permits this action. Mirrors the
    /// transitions enforced by the API so an invalid key press reports the
    /// reason locally instead of round-tripping to a 400.
    fn permitted(self, state: ServerRegistrationState) -> bool {
        match self {
            // A registration must prove itself before it can be accepted.
            ServerAction::Accept => state == ServerRegistrationState::Proven,
            ServerAction::Terminate => state != ServerRegistrationState::Terminated,
            // Deletion drops the record outright, from any state.
            ServerAction::Delete => true,
        }
    }
}

/// An action awaiting a yes/no answer from the user.
struct Confirm {
    action: ServerAction,
    server: ServerRegistration,
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

    // Pending confirmation prompt
    confirm: Option<Confirm>,

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
            confirm: None,
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
                self.sync_inspect_server();
            }
            Err(e) => self.status = format!("Servers: {}", e),
        }
    }

    /// Re-point the inspected server at its freshly loaded copy so the summary
    /// pane reflects state changes. If it is gone from the service, drop out of
    /// the inspect view.
    fn sync_inspect_server(&mut self) {
        let Some(id) = self.inspect_server.as_ref().map(|s| s.id.0) else {
            return;
        };

        match self.service_servers.iter().find(|s| s.id.0 == id) {
            Some(server) => self.inspect_server = Some(server.clone()),
            None => {
                self.inspect_server = None;
                if self.focus == Focus::ServerInspect {
                    self.focus = Focus::Detail;
                }
            }
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

    // -- Registration actions -----------------------------------------------

    /// The registration that action keys apply to in the current view.
    fn action_target(&self) -> Option<&ServerRegistration> {
        match self.focus {
            Focus::ServerInspect => self.inspect_server.as_ref(),
            Focus::Detail if self.detail_tab == DetailTab::Servers => self.selected_detail_server(),
            _ => None,
        }
    }

    /// Raise a confirmation prompt for `action` against the current target.
    fn arm_action(&mut self, action: ServerAction) {
        let Some(server) = self.action_target().cloned() else {
            self.status = "No server selected".into();
            return;
        };

        if !action.permitted(server.state) {
            self.status = format!(
                "Cannot {} a server in state {:?}",
                action.label().to_lowercase(),
                server.state
            );
            return;
        }

        self.confirm = Some(Confirm { action, server });
    }

    /// Run the confirmed action, then reload so the change is visible.
    async fn apply_action(&mut self) {
        let Some(Confirm { action, server }) = self.confirm.take() else {
            return;
        };

        let id = server.id.0;
        self.status = format!("{} {}...", action.progressive(), short_uuid(&id));

        let result = match action {
            ServerAction::Accept => self.client.accept_server().server(id).send().await,
            ServerAction::Terminate => self.client.terminate_server().server(id).send().await,
            ServerAction::Delete => self.client.delete_server().server(id).send().await,
        };

        match result {
            Ok(_) => {
                self.status = format!("{} server {}", action.past(), short_uuid(&id));
                // `load_detail` re-reads the registration list, which drops the
                // inspect view if the server is no longer there.
                self.load_detail().await;
            }
            Err(e) => self.status = format!("{} failed: {}", action.label(), e),
        }
    }

    // -- Input handling -----------------------------------------------------

    /// Returns what data needs loading after this key press.
    fn handle_key(&mut self, key: KeyCode) -> Reload {
        // A confirmation prompt is modal: it swallows every other binding.
        if self.confirm.is_some() {
            return self.handle_confirm_key(key);
        }

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

    fn handle_confirm_key(&mut self, key: KeyCode) -> Reload {
        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => Reload::Action,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Char('q') => {
                self.confirm = None;
                self.status = "Cancelled".into();
                Reload::None
            }
            _ => Reload::None,
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
            KeyCode::Char('a') if self.detail_tab == DetailTab::Servers => {
                self.arm_action(ServerAction::Accept);
                Reload::None
            }
            KeyCode::Char('t') if self.detail_tab == DetailTab::Servers => {
                self.arm_action(ServerAction::Terminate);
                Reload::None
            }
            KeyCode::Char('d') if self.detail_tab == DetailTab::Servers => {
                self.arm_action(ServerAction::Delete);
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
            KeyCode::Char('a') => self.arm_action(ServerAction::Accept),
            KeyCode::Char('t') => self.arm_action(ServerAction::Terminate),
            KeyCode::Char('d') => self.arm_action(ServerAction::Delete),
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

        self.draw_confirm(frame, chunks[1]);
        self.draw_status(frame, chunks[2]);
    }

    fn draw_confirm(&self, frame: &mut Frame, area: Rect) {
        let Some(Confirm { action, server }) = &self.confirm else {
            return;
        };

        let accent = match action {
            ServerAction::Accept => Color::Green,
            ServerAction::Terminate | ServerAction::Delete => Color::Red,
        };
        let bold = Style::default().add_modifier(Modifier::BOLD);

        let body = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" Registration: ", bold),
                Span::raw(server.id.0.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" Instance:     ", bold),
                Span::raw(server.instance_id.0.to_string()),
            ]),
            Line::from(vec![
                Span::styled(" State:        ", bold),
                Span::styled(format!("{:?}", server.state), state_color(server.state)),
            ]),
            Line::raw(""),
            Line::styled(
                format!(" {}", action.description()),
                Style::default().fg(Color::Gray),
            ),
            Line::raw(""),
            Line::from(vec![
                Span::styled(" y", bold),
                Span::raw(": confirm    "),
                Span::styled("n", bold),
                Span::raw("/"),
                Span::styled("Esc", bold),
                Span::raw(": cancel"),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(accent))
                .title(Span::styled(
                    format!(" {} server {}? ", action.label(), short_uuid(&server.id.0)),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                )),
        );

        let popup = centered_rect(area, 76, 9);
        frame.render_widget(Clear, popup);
        frame.render_widget(body, popup);
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
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(pad)),
            Span::styled(right, Style::default().fg(MUTED)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let help = if self.confirm.is_some() {
            " y: confirm | n/Esc: cancel "
        } else {
            match self.focus {
                Focus::Services => " ↑↓: navigate | Enter: details | r: refresh | q: quit ",
                Focus::Detail if self.detail_tab == DetailTab::Servers => {
                    " ↑↓: navigate | ←→: tab | Enter: inspect | a: accept | t: terminate | d: delete | Esc: back | r: refresh "
                }
                Focus::Detail => " ↑↓: navigate | ←→: tab | Esc: back | r: refresh | q: quit ",
                Focus::ServerInspect => {
                    " ↑↓: navigate | ←→: tab | a: accept | t: terminate | d: delete | Esc: back | r: refresh "
                }
            }
        };
        let line = Line::from(vec![
            Span::styled(help, Style::default().fg(MUTED)),
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
        let active = self.focus == Focus::Services;
        let cursor = self.service_state.selected();
        // Drilling into a service fills its dot in: you are now working inside
        // it rather than picking between them.
        let marks = ListMarks {
            cursor: if active { cursor } else { None },
            chosen: if active { None } else { cursor },
        };

        let rows: Vec<Row> = self
            .services
            .iter()
            .enumerate()
            .map(|(i, s)| {
                Row::new(vec![
                    marks.cell(i),
                    Cell::from(s.name.clone()),
                    Cell::from(short_uuid(&s.id.0)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [MARK_COLUMN, Constraint::Min(16), Constraint::Length(8)],
        )
        .header(Row::new(vec!["", "Name", "ID"]).style(header_style()))
        // The Name column flexes to fill the pane, which pushes ID hard against
        // the right border; inset the inner area to give it room.
        .block(pane(" Services ", active).padding(Padding::right(1)))
        .row_highlight_style(marks.row_style());

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
            .block(pane("", self.focus == Focus::Detail))
            .highlight_style(tab_highlight());
        frame.render_widget(tabs, chunks[0]);

        match self.detail_tab {
            DetailTab::Deployments => self.draw_deployments(frame, chunks[1]),
            DetailTab::Servers => self.draw_servers(frame, chunks[1]),
        }
    }

    fn draw_deployments(&mut self, frame: &mut Frame, area: Rect) {
        let active = self.focus == Focus::Detail;
        let marks = ListMarks {
            // Nothing is marked until this pane is the one taking input.
            cursor: if active {
                self.deployment_state.selected()
            } else {
                None
            },
            // There is nothing to open from a deployment, so no dot ever fills.
            chosen: None,
        };

        let rows: Vec<Row> = self
            .deployments
            .iter()
            .enumerate()
            .map(|(i, d)| {
                Row::new(vec![
                    marks.cell(i),
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
                MARK_COLUMN,
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(16),
            ],
        )
        .header(Row::new(vec!["", "ID", "Project", "Silo", "Created"]).style(header_style()))
        .block(pane(" Deployments ", active))
        .row_highlight_style(marks.row_style());

        frame.render_stateful_widget(table, area, &mut self.deployment_state);
    }

    fn draw_servers(&mut self, frame: &mut Frame, area: Rect) {
        let active = self.focus == Focus::Detail;
        let marks = ListMarks {
            // Nothing is marked until this pane is the one taking input.
            cursor: if active {
                self.service_server_state.selected()
            } else {
                None
            },
            // The server being inspected. Inert while the inspect view replaces
            // this pane outright, but it is the rule for the list: open a
            // server and its dot fills in.
            chosen: self.inspect_server.as_ref().and_then(|open| {
                self.service_servers
                    .iter()
                    .position(|s| s.id.0 == open.id.0)
            }),
        };

        let rows: Vec<Row> = self
            .service_servers
            .iter()
            .enumerate()
            .map(|(i, s)| server_row(marks.cell(i), s))
            .collect();

        let table = server_table(rows)
            .block(pane(" Servers (Enter to inspect) ", active))
            .row_highlight_style(marks.row_style());

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
            // Read-only summary: nothing to navigate, so no accent.
            .block(pane(" Server ", false));
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
            .block(pane("", true))
            .highlight_style(tab_highlight());
        frame.render_widget(tabs, chunks[1]);

        match self.inspect_tab {
            InspectTab::Blobs => self.draw_blobs(frame, chunks[2]),
            InspectTab::Checkins => self.draw_checkins(frame, chunks[2]),
        }
    }

    fn draw_blobs(&mut self, frame: &mut Frame, area: Rect) {
        // A leaf list: it takes input the whole time it is on screen, and there
        // is nothing to open from a backup, so no dot ever fills.
        let marks = ListMarks {
            cursor: self.blob_state.selected(),
            chosen: None,
        };

        let rows: Vec<Row> = self
            .blobs
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let state_str = format!("{:?}", b.state);
                Row::new(vec![
                    marks.cell(i),
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
                MARK_COLUMN,
                Constraint::Length(8),
                Constraint::Length(24),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(16),
            ],
        )
        .header(Row::new(vec!["", "ID", "State", "Size", "Total", "Time"]).style(header_style()))
        // The inspect view owns the focus whenever it is on screen.
        .block(pane(format!(" Backups ({}) ", self.blobs.len()), true))
        .row_highlight_style(marks.row_style());

        frame.render_stateful_widget(table, area, &mut self.blob_state);
    }

    fn draw_checkins(&mut self, frame: &mut Frame, area: Rect) {
        let marks = ListMarks {
            cursor: self.checkin_state.selected(),
            chosen: None,
        };

        let rows: Vec<Row> = self
            .checkins
            .iter()
            .enumerate()
            .map(|(i, c)| {
                Row::new(vec![
                    marks.cell(i),
                    Cell::from(short_uuid(&c.id.0)),
                    Cell::from(c.checked_in_at.format("%Y-%m-%d %H:%M:%S").to_string()),
                ])
            })
            .collect();

        // Don't assume the page order: take the newest record we hold.
        let latest = self
            .checkins
            .iter()
            .map(|c| c.checked_in_at)
            .max()
            .map(|at| format!(" Latest: {} ", describe_age(at)))
            .unwrap_or_default();

        let table = Table::new(
            rows,
            [MARK_COLUMN, Constraint::Length(8), Constraint::Min(22)],
        )
        .header(Row::new(vec!["", "ID", "Checked In"]).style(header_style()))
        .block(pane(
            format!(" Checkins ({}){}", self.checkins.len(), latest),
            true,
        ))
        .row_highlight_style(marks.row_style());

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

/// A `width` x `height` rect centered in `area`, clamped to it.
fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Style for a table's column-header row: grey, so the labels sit behind the
/// data instead of competing with it.
fn header_style() -> Style {
    Style::default().fg(MUTED)
}

/// Which rows of one list are marked this frame.
///
/// Two distinct things: the row the cursor sits on, and the row that has been
/// opened. A list whose pane is not taking input has neither, and so shows no
/// selection at all.
struct ListMarks {
    /// Row the cursor sits on, when this list is the one taking input.
    cursor: Option<usize>,
    /// Row that has been opened — the service you are working inside, the
    /// server you are inspecting. Takes precedence over the cursor.
    chosen: Option<usize>,
}

impl ListMarks {
    /// Cell for the marker column: an open dot on the cursor row, filled once
    /// that row has been opened, blank everywhere else.
    fn cell(&self, index: usize) -> Cell<'static> {
        let glyph = if self.chosen == Some(index) {
            "●"
        } else if self.cursor == Some(index) {
            "○"
        } else {
            " "
        };

        Cell::from(Line::styled(glyph, Style::default().fg(MARK)).centered())
    }

    /// Style to patch over the marked row.
    ///
    /// Bold, and only when this list marks something — a list whose pane is
    /// inactive leaves its rows alone. It sets no foreground or background on
    /// purpose: ratatui patches `row_highlight_style` over cells that have
    /// already been drawn, so a colour here would flatten the State column's
    /// own colour on the very row you are looking at.
    fn row_style(&self) -> Style {
        if self.cursor.is_some() || self.chosen.is_some() {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        }
    }
}

/// A bordered pane whose border carries the accent colour when it holds focus.
fn pane<'a, T: Into<Line<'a>>>(title: T, focused: bool) -> Block<'a> {
    let border = if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default()
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(title)
}

/// Style for the selected entry of a tab bar.
fn tab_highlight() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
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

fn server_row(marker: Cell<'static>, s: &ServerRegistration) -> Row<'static> {
    Row::new(vec![
        marker,
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
            MARK_COLUMN,
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(16),
        ],
    )
    .header(
        Row::new(vec![
            "", "ID", "Instance", "State", "Project", "Silo", "Updated",
        ])
        .style(header_style()),
    )
}

fn short_uuid(uuid: &uuid::Uuid) -> String {
    uuid.to_string()[..8].to_string()
}

/// How long ago `then` was, relative to this machine's clock.
///
/// A check-in is timestamped by the guest that sent it (`sprue-agent` stamps
/// `checked_in_at` from its own clock), so a guest running ahead of the viewer
/// puts the timestamp in the future. Say so rather than reporting a negative
/// age, since the difference is clock skew and not elapsed time.
fn describe_age(then: DateTime<Utc>) -> String {
    let seconds = (Utc::now() - then).num_seconds();
    match seconds {
        // Sub-second skew is just latency and rounding; don't make noise of it.
        -1..=1 => "just now".to_string(),
        s if s < 0 => format!("{} ahead (clock skew)", format_duration(-s)),
        s => format!("{} ago", format_duration(s)),
    }
}

/// Coarse duration for a non-negative number of seconds: two units, largest
/// first, so the width stays roughly stable as the value grows.
fn format_duration(seconds: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    match seconds {
        s if s >= DAY => format!("{}d {}h", s / DAY, (s % DAY) / HOUR),
        s if s >= HOUR => format!("{}h {}m", s / HOUR, (s % HOUR) / MINUTE),
        s if s >= MINUTE => format!("{}m {}s", s / MINUTE, s % MINUTE),
        s => format!("{}s", s),
    }
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
    // `init` enables raw mode, enters the alternate screen, and installs a panic
    // hook that restores both; `restore` undoes it. Doing any of that by hand as
    // well leaves the terminal in raw mode on exit.
    let terminal = ratatui::try_init()?;
    let result = run_app(terminal, client).await;
    ratatui::restore();

    result
}

async fn run_app(mut terminal: DefaultTerminal, client: Client) -> io::Result<()> {
    let mut app = App::new(client);
    app.refresh_all().await;
    let mut last_service_idx = app.service_state.selected();

    loop {
        terminal.draw(|f| app.draw(f))?;

        // Auto-refresh, held off while a confirmation prompt is up so the
        // registration being acted on cannot shift underneath it.
        if app.confirm.is_none() && Instant::now() >= app.next_refresh {
            app.refresh_all().await;
            last_service_idx = app.service_state.selected();
            continue;
        }

        if event::poll(EVENT_POLL)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.code == KeyCode::Char('r') && app.confirm.is_none() {
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
                    Reload::Action => {
                        app.apply_action().await;
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
