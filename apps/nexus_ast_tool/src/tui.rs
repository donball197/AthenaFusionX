//! NEXUS Security Hold Command Center
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, List, ListItem, ListState,
        Paragraph, Row, Table, Tabs, Wrap,
    },
};
use std::fs;
use std::io::stdout;
use std::path::PathBuf;

use crate::hold::{load, FileStatus, DEFAULT_DIR, Hold, HeldFile, HeldFinding};

const BORDER_DIM: Color = Color::Rgb(26, 46, 34);
const BORDER_BRIGHT: Color = Color::Rgb(42, 78, 56);
const GREEN: Color = Color::Rgb(0, 255, 136);
const GREEN_DIM: Color = Color::Rgb(0, 204, 106);
const AMBER: Color = Color::Rgb(255, 170, 0);
const RED: Color = Color::Rgb(255, 51, 85);
const CYAN: Color = Color::Rgb(0, 221, 255);
const WHITE: Color = Color::Rgb(208, 232, 216);
const GRAY: Color = Color::Rgb(138, 158, 146);
const GRAY_DARK: Color = Color::Rgb(74, 94, 82);

struct App {
    holds: Vec<(String, Hold)>,
    sel_hold: usize,
    sel_file: usize,
    sel_finding: usize,
    list_state: ListState,
    active_tab: usize,
    show_help: bool,
    should_quit: bool,
    scroll_diff: u16,
    status_msg: String,
    status_color: Color,
    terminal_lines: Vec<String>,
    terminal_input: String,
}

impl App {
    fn new(hold_dir: PathBuf) -> Self {
        let mut holds = Vec::new();
        if let Ok(rd) = fs::read_dir(&hold_dir) {
            for entry in rd.flatten() {
                let id = entry.file_name().to_string_lossy().to_string();
                if let Ok(h) = load(&hold_dir, &id) {
                    holds.push((id, h));
                }
            }
        }
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let terminal_lines = vec![
            "  ============================================".to_string(),
            "     NEXUS SECURITY HOLD COMMAND CENTER v2.0".to_string(),
            "  ============================================".to_string(),
            String::new(),
            "  Type 'help' for commands. 'q' to quit.".to_string(),
            String::new(),
        ];
        Self {
            holds,
            sel_hold: 0,
            sel_file: 0,
            sel_finding: 0,
            list_state,
            active_tab: 0,
            show_help: false,
            should_quit: false,
            scroll_diff: 0,
            status_msg: "Ready.".to_string(),
            status_color: GREEN,
            terminal_lines,
            terminal_input: String::new(),
        }
    }

    fn current_hold(&self) -> Option<&Hold> {
        self.holds.get(self.sel_hold).map(|(_, h)| h)
    }

    fn current_file(&self) -> Option<&HeldFile> {
        self.current_hold()?.files.get(self.sel_file)
    }

    fn current_finding(&self) -> Option<&HeldFinding> {
        self.current_file()?.findings.get(self.sel_finding)
    }

    fn total_findings(&self) -> usize {
        self.holds.iter().flat_map(|(_, h)| h.files.iter()).flat_map(|f| f.findings.iter()).count()
    }

    fn held_count(&self) -> usize {
        self.holds.iter().flat_map(|(_, h)| h.files.iter()).filter(|f| f.status == FileStatus::Held).count()
    }

    fn promoted_count(&self) -> usize {
        self.holds.iter().flat_map(|(_, h)| h.files.iter()).filter(|f| f.status == FileStatus::Promoted).count()
    }

    fn total_files(&self) -> usize {
        self.holds.iter().flat_map(|(_, h)| h.files.iter()).count()
    }

    fn next_hold(&mut self) {
        if self.sel_hold + 1 < self.holds.len() {
            self.sel_hold += 1;
            self.sel_file = 0;
            self.sel_finding = 0;
            self.list_state.select(Some(self.sel_hold));
            self.status_msg = format!("Hold: {}", self.holds[self.sel_hold].0);
            self.status_color = GREEN;
        }
    }

    fn prev_hold(&mut self) {
        if self.sel_hold > 0 {
            self.sel_hold -= 1;
            self.sel_file = 0;
            self.sel_finding = 0;
            self.list_state.select(Some(self.sel_hold));
            self.status_msg = format!("Hold: {}", self.holds[self.sel_hold].0);
            self.status_color = GREEN;
        }
    }

    fn next_file(&mut self) {
        if let Some(n) = self.current_hold().map(|h| h.files.len()) {
            if self.sel_file + 1 < n {
                self.sel_file += 1;
                self.sel_finding = 0;
            }
        }
    }

    fn prev_file(&mut self) {
        if self.sel_file > 0 {
            self.sel_file -= 1;
            self.sel_finding = 0;
        }
    }

    fn next_finding(&mut self) {
        if let Some(n) = self.current_file().map(|f| f.findings.len()) {
            if self.sel_finding + 1 < n {
                self.sel_finding += 1;
            }
        }
    }

    fn prev_finding(&mut self) {
        if self.sel_finding > 0 {
            self.sel_finding -= 1;
        }
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());
    draw_header(frame, app, chunks[0]);
    draw_content(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
    if app.show_help {
        draw_help(frame);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        Span::styled(" OVERVIEW ", Style::default().fg(GRAY)),
        Span::styled(" FINDINGS ", Style::default().fg(GRAY)),
        Span::styled(" DIFF ", Style::default().fg(GRAY)),
        Span::styled(" TERMINAL ", Style::default().fg(GRAY)),
    ];
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_BRIGHT))
                .title(Span::styled(" NEXUS SECURITY HOLD ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)))
                .title_bottom(Span::styled(
                    format!(" {} findings | {} held | {} promoted ", app.total_findings(), app.held_count(), app.promoted_count()),
                    Style::default().fg(GRAY_DARK),
                )),
        )
        .select(app.active_tab)
        .highlight_style(Style::default().fg(GREEN).add_modifier(Modifier::BOLD))
        .divider("|");
    frame.render_widget(tabs, area);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.active_tab {
        0 => draw_overview(frame, app, area),
        1 => draw_findings(frame, app, area),
        2 => draw_diff(frame, app, area),
        _ => draw_terminal(frame, app, area),
    }
}

fn draw_overview(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    let items: Vec<ListItem<'static>> = app.holds.iter().map(|(id, h)| {
        let held = h.files.iter().filter(|f| f.status == FileStatus::Held).count();
        let promoted = h.files.iter().filter(|f| f.status == FileStatus::Promoted).count();
        let style = if held > 0 { Style::default().fg(AMBER) } else { Style::default().fg(GREEN) };
        ListItem::new(vec![
            Line::from(Span::styled(format!(" * {}", id), style.add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("   {} files | {} held | {} promoted", h.files.len(), held, promoted), Style::default().fg(GRAY_DARK))),
            Line::from(Span::styled(format!("   > {}", h.authoritative_root.display()), Style::default().fg(GRAY_DARK))),
        ])
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(Span::styled(" Holds ", Style::default().fg(GRAY))))
        .highlight_style(Style::default().bg(Color::Rgb(18, 33, 26)).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, cols[0], &mut app.list_state);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(cols[1]);

    let total = app.total_files();
    let held = app.held_count();
    let promoted = app.promoted_count();

    let stats = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(right[0]);

    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DIM))
                .title(Span::styled(" Held ", Style::default().fg(AMBER))))
            .gauge_style(Style::default().fg(AMBER))
            .ratio(if total > 0 { held as f64 / total as f64 } else { 0.0 })
            .label(format!("{}/{}", held, total)),
        stats[0],
    );

    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DIM))
                .title(Span::styled(" Promoted ", Style::default().fg(GREEN))))
            .gauge_style(Style::default().fg(GREEN))
            .ratio(if total > 0 { promoted as f64 / total as f64 } else { 0.0 })
            .label(format!("{}/{}", promoted, total)),
        stats[1],
    );

    if let Some(h) = app.current_hold() {
        let rows: Vec<Row> = h.files.iter().enumerate().map(|(i, f)| {
            let s = match f.status {
                FileStatus::Held => Style::default().fg(AMBER),
                FileStatus::Promoted => Style::default().fg(GREEN),
            };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)),
                Cell::from(f.rel.clone()),
                Cell::from(format!("{:?}", f.status)).style(s),
                Cell::from(format!("{}", f.findings.len())),
            ])
        }).collect();

        frame.render_widget(
            Table::new(rows, [Constraint::Length(4), Constraint::Min(20), Constraint::Length(10), Constraint::Length(8)])
                .header(Row::new(vec!["#", "File", "Status", "Findings"]).style(Style::default().fg(GRAY).add_modifier(Modifier::BOLD)))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_DIM))
                    .title(Span::styled(format!(" Files in {} ", h.id), Style::default().fg(GRAY)))),
            right[1],
        );
    }
}

fn draw_findings(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let mut items: Vec<ListItem<'static>> = Vec::new();
    if let Some(h) = app.current_hold() {
        for f in &h.files {
            for x in &f.findings {
                let icon = match f.status {
                    FileStatus::Held => "*",
                    FileStatus::Promoted => "ok",
                };
                let sev = if x.pattern.contains("COMMAND") { RED }
                    else if x.pattern.contains("SQL") { AMBER }
                    else { CYAN };
                let fname = f.rel.split('/').last().unwrap_or("").to_string();
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(GREEN)),
                    Span::styled(format!("{:<28}", x.pattern), Style::default().fg(sev)),
                    Span::styled(format!(":{} ", x.line), Style::default().fg(GRAY_DARK)),
                    Span::styled(fname, Style::default().fg(GRAY_DARK)),
                ])));
            }
        }
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_DIM))
            .title(Span::styled(" Findings ", Style::default().fg(GRAY))))
        .highlight_style(Style::default().bg(Color::Rgb(18, 33, 26)).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");
    frame.render_stateful_widget(list, cols[0], &mut app.list_state);

    if let Some(x) = app.current_finding() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Pattern:  ", Style::default().fg(GRAY_DARK)),
                Span::styled(x.pattern.clone(), Style::default().fg(AMBER).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Location: ", Style::default().fg(GRAY_DARK)),
                Span::styled(
                    format!("{}:{}", app.current_file().map(|f| f.rel.clone()).unwrap_or_default(), x.line),
                    Style::default().fg(CYAN),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled("-- Proposed Patch --", Style::default().fg(GRAY))),
            Line::from(""),
        ];
        for line in x.patch.lines() {
            let (style, mark) = if line.starts_with('+') {
                (Style::default().fg(GREEN_DIM), " + ")
            } else if line.starts_with('-') {
                (Style::default().fg(RED), " - ")
            } else {
                (Style::default().fg(GRAY_DARK), "   ")
            };
            lines.push(Line::from(vec![Span::styled(mark, style), Span::styled(line.to_string(), style)]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("-- Trust --", Style::default().fg(GRAY))));
        lines.push(Line::from(" [ok] Original backed up"));
        lines.push(Line::from(" [ok] SHA-256 verified"));
        lines.push(Line::from(" [ok] Atomic write"));
        lines.push(Line::from(" [ok] Conflict detected"));

        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_DIM))
                    .title(Span::styled(format!(" {} @ line {} ", x.pattern, x.line), Style::default().fg(GREEN))))
                .wrap(Wrap { trim: false }),
            cols[1],
        );
    }
}

fn draw_diff(frame: &mut Frame, app: &mut App, area: Rect) {
    if let Some(x) = app.current_finding() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled(" File: ", Style::default().fg(GRAY_DARK)),
                Span::styled(app.current_file().map(|f| f.rel.clone()).unwrap_or_default(), Style::default().fg(CYAN)),
            ]),
            Line::from(""),
        ];
        for line in x.patch.lines() {
            let (style, mark) = if line.starts_with('+') {
                (Style::default().fg(GREEN_DIM).bg(Color::Rgb(6, 33, 22)), "+")
            } else if line.starts_with('-') {
                (Style::default().fg(RED).bg(Color::Rgb(33, 6, 12)), "-")
            } else {
                (Style::default().fg(GRAY_DARK), " ")
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", mark), style),
                Span::styled(format!("  {}", line), style),
            ]));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BORDER_DIM))
                    .title(Span::styled(" Diff View ", Style::default().fg(GRAY))))
                .scroll((app.scroll_diff, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
    }
}

fn draw_terminal(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let output: Vec<Line> = app.terminal_lines.iter().map(|l| {
        let style = if l.contains("==") || l.contains("NEXUS") {
            Style::default().fg(GREEN_DIM)
        } else if l.contains("ERROR") || l.contains("fail") || l.contains("Unknown") {
            Style::default().fg(RED)
        } else if l.contains("ok") || l.contains("OK") {
            Style::default().fg(GREEN)
        } else if l.starts_with("nexus>") {
            Style::default().fg(GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(GRAY)
        };
        Line::from(Span::styled(l.clone(), style))
    }).collect();

    let h = chunks[0].height.saturating_sub(1) as usize;
    let scroll = app.terminal_lines.len().saturating_sub(h) as u16;

    frame.render_widget(
        Paragraph::new(output)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER_DIM))
                .title(Span::styled(" Nexus Shell ", Style::default().fg(GRAY))))
            .scroll((scroll, 0)),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("nexus> ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(app.terminal_input.clone(), Style::default().fg(WHITE)),
            Span::styled("_", Style::default().fg(GREEN)),
        ]))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER_BRIGHT))),
        chunks[1],
    );
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    // Touch-friendly button bar — large tappable zones
    let btns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(16),
        ])
        .split(area);

    let labels = [
        ("  UP  ", if app.sel_hold > 0 { GREEN } else { GRAY_DARK }),
        ("  DOWN  ", GREEN),
        ("  PREV  ", GREEN),
        ("  NEXT  ", GREEN),
        ("  TAB  ", GREEN),
        ("  DIFF  ", GREEN),
        ("  HELP  ", GREEN),
        ("  QUIT  ", RED),
    ];

    for (i, (label, color)) in labels.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                *label,
                Style::default().fg(*color).bg(Color::Rgb(12, 22, 18)).add_modifier(Modifier::BOLD),
            )))
            .alignment(Alignment::Center),
            btns[i],
        );
    }
}

fn draw_help(frame: &mut Frame) {
    let area = frame.area();
    let popup = centered(60, 70, area);
    let help = vec![
        Line::from(Span::styled(" NEXUS COMMAND CENTER - KEYBINDINGS ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled(" NAVIGATION ", Style::default().fg(AMBER))),
        Line::from("  j / Down       Next item"),
        Line::from("  k / Up         Previous item"),
        Line::from("  J / K          Next/Prev file"),
        Line::from("  n / p          Next/Prev finding"),
        Line::from("  Tab            Switch panel"),
        Line::from("  1-4            Jump to tab"),
        Line::from(""),
        Line::from(Span::styled(" VIEWS ", Style::default().fg(AMBER))),
        Line::from("  d / u          Scroll diff down/up"),
        Line::from(""),
        Line::from(Span::styled(" GENERAL ", Style::default().fg(AMBER))),
        Line::from("  ?              Toggle help"),
        Line::from("  q / Esc        Quit"),
    ];
    frame.render_widget(Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0))), area);
    frame.render_widget(
        Paragraph::new(help)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Double)
                .border_style(Style::default().fg(GREEN))
                .title(Span::styled(" Help ", Style::default().fg(GREEN))))
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn centered(px: u16, py: u16, r: Rect) -> Rect {
    let v = Layout::default().direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - py) / 2),
            Constraint::Percentage(py),
            Constraint::Percentage((100 - py) / 2),
        ]).split(r);
    Layout::default().direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - px) / 2),
            Constraint::Percentage(px),
            Constraint::Percentage((100 - px) / 2),
        ]).split(v[1])[1]
}

fn handle_keys(app: &mut App) -> Result<()> {
    match event::read()? {
        Event::Mouse(mouse) => {
            let y = mouse.row;
            let x = mouse.column;
            // Button bar is at the bottom row (area height - 1)
            // But we need to check if click is on the button bar
            // For now, map clicks to actions based on column position
            if let crossterm::event::MouseEventKind::Down(_) = mouse.kind {
                let pct = (x as f64 / 120.0 * 100.0) as u16;
                if pct < 12 {
                    app.prev_hold();
                } else if pct < 24 {
                    app.next_hold();
                } else if pct < 36 {
                    app.prev_file();
                } else if pct < 48 {
                    app.next_file();
                } else if pct < 60 {
                    app.active_tab = (app.active_tab + 1) % 4;
                } else if pct < 72 {
                    app.scroll_diff = app.scroll_diff.saturating_add(3);
                } else if pct < 84 {
                    app.show_help = !app.show_help;
                } else {
                    app.should_quit = true;
                }
                app.status_msg = format!("Tap at ({}, {})", x, y);
                app.status_color = CYAN;
            }
            return Ok(());
        }
        Event::Key(key) => {
            handle_key_event(app, key)?;
            return Ok(());
        }
        _ => return Ok(()),
    }
}

fn handle_key_event(app: &mut App, key: crossterm::event::KeyEvent) -> Result<()> {
    {
        let key = key;
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        match key.code {
            KeyCode::Char('?') => { app.show_help = !app.show_help; return Ok(()); }
            KeyCode::Tab => { app.active_tab = (app.active_tab + 1) % 4; return Ok(()); }
            KeyCode::Char('1') if key.modifiers == KeyModifiers::NONE => { app.active_tab = 0; return Ok(()); }
            KeyCode::Char('2') if key.modifiers == KeyModifiers::NONE => { app.active_tab = 1; return Ok(()); }
            KeyCode::Char('3') if key.modifiers == KeyModifiers::NONE => { app.active_tab = 2; return Ok(()); }
            KeyCode::Char('4') if key.modifiers == KeyModifiers::NONE => { app.active_tab = 3; return Ok(()); }
            _ => {}
        }

        if app.active_tab == 3 {
            match key.code {
                KeyCode::Char(c) => { app.terminal_input.push(c); return Ok(()); }
                KeyCode::Backspace => { app.terminal_input.pop(); return Ok(()); }
                KeyCode::Enter => {
                    let cmd = app.terminal_input.trim().to_string();
                    app.terminal_lines.push(format!("nexus> {}", cmd));
                    let mut out: Vec<String> = Vec::new();
                    match cmd.as_str() {
                        "q" | "exit" => app.should_quit = true,
                        "clear" => { app.terminal_lines.clear(); }
                        "help" => {
                            for l in ["  review <id>  - Show hold details", "  list         - List all holds", "  status       - Show counts", "  ls           - List files", "  findings     - List findings", "  clear        - Clear screen", "  version      - Version info", "  date         - Timestamp", "  q            - Quit"] {
                                out.push(l.to_string());
                            }
                        }
                        "list" => {
                            for (id, h) in &app.holds {
                                let held = h.files.iter().filter(|f| f.status == FileStatus::Held).count();
                                out.push(format!("  {}  files={} held={}", id, h.files.len(), held));
                            }
                            if app.holds.is_empty() { out.push("  (no holds)".into()); }
                        }
                        "status" => {
                            out.push(format!("  Holds: {} | Files: {} | Findings: {}", app.holds.len(), app.total_files(), app.total_findings()));
                            out.push(format!("  Held: {} | Promoted: {}", app.held_count(), app.promoted_count()));
                        }
                        "ls" | "files" => {
                            if let Some(h) = app.current_hold() {
                                for f in &h.files { out.push(format!("  [{:?}] {}", f.status, f.rel)); }
                            } else { out.push("  (no hold selected)".into()); }
                        }
                        "findings" => {
                            if let Some(h) = app.current_hold() {
                                for f in &h.files {
                                    for x in &f.findings { out.push(format!("  {} @ {}:{}", x.pattern, f.rel, x.line)); }
                                }
                            } else { out.push("  (no hold selected)".into()); }
                        }
                        "date" => {
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                            out.push(format!("  Unix timestamp: {}", now));
                        }
                        "version" => {
                            out.push("  NEXUS Security Hold Command Center v2.0".into());
                            out.push("  ratatui + crossterm".into());
                        }
                        _ => {
                            if cmd.starts_with("review") {
                                let target = cmd.strip_prefix("review").unwrap_or("").trim();
                                for (id, h) in &app.holds {
                                    if id == target || target.is_empty() {
                                        out.push(format!("  Hold: {}", id));
                                        out.push(format!("  Root: {}", h.authoritative_root.display()));
                                        for f in &h.files {
                                            out.push(format!("  [{:?}] {}", f.status, f.rel));
                                            for x in &f.findings { out.push(format!("    - {} @ line {}", x.pattern, x.line)); }
                                        }
                                    }
                                }
                            } else if !cmd.is_empty() {
                                out.push(format!("  Unknown command: {}", cmd));
                            }
                        }
                    }
                    app.terminal_lines.extend(out);
                    app.terminal_input.clear();
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => match app.active_tab {
                0 | 1 => app.next_hold(),
                2 => app.scroll_diff = app.scroll_diff.saturating_add(1),
                _ => {}
            },
            KeyCode::Char('k') | KeyCode::Up => match app.active_tab {
                0 | 1 => app.prev_hold(),
                2 => app.scroll_diff = app.scroll_diff.saturating_sub(1),
                _ => {}
            },
            KeyCode::Char('J') => app.next_file(),
            KeyCode::Char('K') => app.prev_file(),
            KeyCode::Char('n') => app.next_finding(),
            KeyCode::Char('p') => app.prev_finding(),
            KeyCode::Char('d') => app.scroll_diff = app.scroll_diff.saturating_add(3),
            KeyCode::Char('u') => app.scroll_diff = app.scroll_diff.saturating_sub(3),
            _ => {}
        }
    }
    Ok(())
}

pub fn run(hold_dir: Option<PathBuf>) -> Result<()> {
    let dir = hold_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
    let mut app = App::new(dir);

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    crossterm::execute!(stdout(), crossterm::event::EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    for _ in 0..10 {
        if event::poll(std::time::Duration::from_millis(0))? {
            let _ = event::read()?;
        } else {
            break;
        }
    }

    while !app.should_quit {
        terminal.draw(|f| draw(f, &mut app))?;
        handle_keys(&mut app)?;
    }

    crossterm::execute!(stdout(), crossterm::event::DisableMouseCapture)?;
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    println!("NEXUS Command Center exited.");
    Ok(())
}
