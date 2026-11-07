use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    app::{App, Focus, Mode, Overlay, ParamField},
    editor::TextInput,
    model::Method,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

pub fn draw(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_header(frame, app, root[0]);
    draw_main(frame, app, root[1]);
    draw_footer(frame, app, root[2]);
    draw_overlay(frame, app);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let workspace = &app.workspaces[app.active_workspace];
    let content = Line::from(vec![
        Span::styled(
            " restui ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  Workspace "),
        Span::styled(&workspace.name, Style::default().fg(ACCENT).bold()),
        Span::styled(
            format!(
                "  ·  {} request{}",
                workspace.requests.len(),
                if workspace.requests.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            Style::default().fg(MUTED),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(content).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(area);
    draw_requests(frame, app, columns[0]);

    let request_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(5),
        ])
        .split(columns[1]);
    draw_url_bar(frame, app, request_area[0]);

    let editors = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(request_area[1]);
    draw_params(frame, app, editors[0]);
    draw_body(frame, app, editors[1]);
    draw_response(frame, app, request_area[2]);
}

fn draw_requests(frame: &mut Frame, app: &App, area: Rect) {
    let requests = &app.workspaces[app.active_workspace].requests;
    let items: Vec<ListItem> = requests
        .iter()
        .map(|request| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<7}", request.method.as_str()),
                    method_style(request.method),
                ),
                Span::raw(&request.name),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(focused_block(" Requests ", app.focus == Focus::Requests))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");
    let mut state = ListState::default().with_selected(Some(app.selected_request));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_url_bar(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(13), Constraint::Min(10)])
        .split(area);
    let method = Paragraph::new(Line::from(vec![Span::styled(
        format!("‹ {} ›", app.current_request().method.as_str()),
        method_style(app.current_request().method).add_modifier(Modifier::BOLD),
    )]))
    .alignment(Alignment::Center)
    .block(focused_block(" Method ", app.focus == Focus::Method));
    frame.render_widget(method, columns[0]);
    draw_input(
        frame,
        columns[1],
        " URL ",
        &app.url_input,
        app.focus == Focus::Url,
        app.mode == Mode::EditUrl,
    );
}

fn draw_params(frame: &mut Frame, app: &App, area: Rect) {
    let request = app.current_request();
    let mut lines = Vec::new();
    if request.params.is_empty() {
        lines.push(Line::styled(
            "No query parameters",
            Style::default().fg(MUTED),
        ));
    } else {
        for param in &request.params {
            lines.push(Line::from(vec![
                Span::styled(
                    if param.enabled { "[x] " } else { "[ ] " },
                    Style::default().fg(ACCENT),
                ),
                Span::styled(&param.key, Style::default().bold()),
                Span::raw(" = "),
                Span::raw(&param.value),
            ]));
        }
    }
    lines.push(Line::styled(
        "Enter to edit",
        Style::default().fg(MUTED).add_modifier(Modifier::ITALIC),
    ));
    frame.render_widget(
        Paragraph::new(lines).block(focused_block(
            &format!(" Query Params ({}) ", request.params.len()),
            app.focus == Focus::Params,
        )),
        area,
    );
}

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    let editing = app.mode == Mode::EditBody;
    let title = if app.current_request().method.supports_body() {
        " Body · JSON "
    } else {
        " Body · used by POST/PUT/PATCH "
    };
    let block = focused_block(title, app.focus == Focus::Body);
    let inner = block.inner(area);
    let (row, column) = app.body_input.row_column();
    let vertical = if editing {
        (row + 1).saturating_sub(inner.height as usize) as u16
    } else {
        0
    };
    let horizontal = if editing {
        column.saturating_sub((inner.width as usize).saturating_sub(1)) as u16
    } else {
        0
    };
    let body = if app.body_input.value().is_empty() && !editing {
        "Enter to edit JSON body"
    } else {
        app.body_input.value()
    };
    let style = if body.is_empty() || body == "Enter to edit JSON body" {
        Style::default().fg(MUTED)
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(body)
            .style(style)
            .block(block)
            .scroll((vertical, horizontal)),
        area,
    );
    if editing && inner.width > 0 && inner.height > 0 {
        frame.set_cursor_position((
            inner.x + (column.saturating_sub(horizontal as usize)) as u16,
            inner.y + (row.saturating_sub(vertical as usize)) as u16,
        ));
    }
}

fn draw_response(frame: &mut Frame, app: &App, area: Rect) {
    let (title, lines) = if app.is_sending() {
        (
            " Response · sending… ".to_string(),
            vec![Line::styled(
                "Waiting for the server",
                Style::default().fg(MUTED),
            )],
        )
    } else {
        match app.current_response() {
            Some(Ok(response)) => {
                let title = format!(
                    " Response · {} {} · {} ms · {} B{} ",
                    response.status.as_u16(),
                    response.status.canonical_reason().unwrap_or(""),
                    response.elapsed.as_millis(),
                    response.bytes,
                    if response.is_json { " · JSON" } else { "" }
                );
                let lines = if response.display_body.is_empty() {
                    vec![Line::styled(
                        "Empty response body",
                        Style::default().fg(MUTED),
                    )]
                } else {
                    response
                        .display_body
                        .lines()
                        .map(|line| {
                            if response.is_json {
                                highlight_json(line)
                            } else {
                                Line::raw(line)
                            }
                        })
                        .collect()
                };
                (title, lines)
            }
            Some(Err(error)) => (
                " Response · error ".to_string(),
                vec![Line::styled(
                    error.as_str(),
                    Style::default().fg(Color::Red),
                )],
            ),
            None => (
                " Response ".to_string(),
                vec![Line::styled(
                    "Ctrl+S to send · y to copy · arrows to scroll",
                    Style::default().fg(MUTED),
                )],
            ),
        }
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(focused_block(&title, app.focus == Focus::Response))
            .scroll(app.response_scroll),
        area,
    );
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let help = match app.mode {
        Mode::EditUrl => "Type URL · Enter/Esc save · Tab params · Ctrl+S send · Ctrl+Q quit",
        Mode::EditBody => {
            "Edit body · arrows/Home/End navigate · Tab indent · Esc save · Ctrl+S send"
        }
        Mode::Normal => match app.overlay {
            Some(Overlay::Workspaces) => "↑/↓ select · Enter switch · n new workspace · Esc close",
            Some(Overlay::Params) => "a add · Enter/e edit · Space toggle · d delete · Esc close",
            Some(Overlay::ParamEditor { .. }) => {
                "Tab switch field · Enter save parameter · Esc cancel"
            }
            Some(Overlay::NewWorkspace(_) | Overlay::Import(_) | Overlay::Rename(_)) => {
                "Type text · Enter confirm · Esc cancel"
            }
            None => {
                "Ctrl+S send · Ctrl+N new · Ctrl+W workspaces · Ctrl+O/i import · Enter edit · Tab focus · y copy · r rename · q quit"
            }
        },
    };
    let lines = vec![
        Line::styled(help, Style::default().fg(MUTED)),
        Line::styled(&app.status_message, Style::default().fg(ACCENT)),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn draw_overlay(frame: &mut Frame, app: &App) {
    let Some(overlay) = &app.overlay else {
        return;
    };
    match overlay {
        Overlay::Workspaces => {
            let area = centered(frame.area(), 55, 60);
            frame.render_widget(Clear, area);
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(area);
            let items: Vec<ListItem> = app
                .workspaces
                .iter()
                .map(|workspace| {
                    ListItem::new(format!(
                        "{}  ({} requests)",
                        workspace.name,
                        workspace.requests.len()
                    ))
                })
                .collect();
            let list = List::new(items)
                .block(Block::bordered().title(" Workspaces "))
                .highlight_style(Style::default().bg(Color::DarkGray).bold())
                .highlight_symbol("› ");
            let mut state = ListState::default().with_selected(Some(app.workspace_selected));
            frame.render_stateful_widget(list, layout[0], &mut state);
            frame.render_widget(
                Paragraph::new("n new · Enter switch · Esc close").alignment(Alignment::Center),
                layout[1],
            );
        }
        Overlay::NewWorkspace(input) => {
            draw_line_dialog(frame, " New Workspace ", "Name", input);
        }
        Overlay::Import(input) => {
            draw_line_dialog(frame, " Import Collection ", "JSON file path", input);
        }
        Overlay::Rename(input) => {
            draw_line_dialog(frame, " Rename Request ", "Name", input);
        }
        Overlay::Params => {
            let area = centered(frame.area(), 75, 65);
            frame.render_widget(Clear, area);
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(1)])
                .split(area);
            let items: Vec<ListItem> = app
                .current_request()
                .params
                .iter()
                .map(|param| {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            if param.enabled { "[x] " } else { "[ ] " },
                            Style::default().fg(ACCENT),
                        ),
                        Span::styled(&param.key, Style::default().bold()),
                        Span::raw(" = "),
                        Span::raw(&param.value),
                    ]))
                })
                .collect();
            let list = List::new(items)
                .block(Block::bordered().title(" Query Parameters "))
                .highlight_style(Style::default().bg(Color::DarkGray).bold())
                .highlight_symbol("› ");
            let selected = if app.current_request().params.is_empty() {
                None
            } else {
                Some(
                    app.param_selected
                        .min(app.current_request().params.len() - 1),
                )
            };
            let mut state = ListState::default().with_selected(selected);
            frame.render_stateful_widget(list, layout[0], &mut state);
            frame.render_widget(
                Paragraph::new("a add · e edit · Space toggle · d delete · Esc close")
                    .alignment(Alignment::Center),
                layout[1],
            );
        }
        Overlay::ParamEditor {
            key,
            value,
            field,
            index,
        } => {
            let area = centered(frame.area(), 60, 30);
            frame.render_widget(Clear, area);
            let outer = Block::bordered().title(if index.is_some() {
                " Edit Parameter "
            } else {
                " Add Parameter "
            });
            let inner = outer.inner(area);
            frame.render_widget(outer, area);
            let fields = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ])
                .split(inner);
            draw_input(
                frame,
                fields[0],
                " Key ",
                key,
                true,
                *field == ParamField::Key,
            );
            draw_input(
                frame,
                fields[1],
                " Value ",
                value,
                true,
                *field == ParamField::Value,
            );
            frame.render_widget(
                Paragraph::new("Tab switch · Enter save · Esc cancel")
                    .style(Style::default().fg(MUTED))
                    .alignment(Alignment::Center),
                fields[2],
            );
        }
    }
}

fn draw_line_dialog(frame: &mut Frame, title: &str, label: &str, input: &TextInput) {
    let area = centered(frame.area(), 65, 20);
    frame.render_widget(Clear, area);
    let outer = Block::bordered().title(title);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);
    draw_input(frame, layout[0], &format!(" {label} "), input, true, true);
    frame.render_widget(
        Paragraph::new("Enter confirm · Esc cancel")
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center),
        layout[1],
    );
}

fn draw_input(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    input: &TextInput,
    focused: bool,
    editing: bool,
) {
    let block = focused_block(title, focused);
    let inner = block.inner(area);
    let (_, column) = input.row_column();
    let horizontal = column.saturating_sub((inner.width as usize).saturating_sub(1)) as u16;
    frame.render_widget(
        Paragraph::new(input.value())
            .block(block)
            .scroll((0, horizontal)),
        area,
    );
    if editing && inner.width > 0 && inner.height > 0 {
        frame.set_cursor_position((
            inner.x + column.saturating_sub(horizontal as usize) as u16,
            inner.y,
        ));
    }
}

fn focused_block<'a>(title: &'a str, focused: bool) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(ACCENT)
        } else {
            Style::default().fg(MUTED)
        })
}

fn method_style(method: Method) -> Style {
    let color = match method {
        Method::Get => Color::Green,
        Method::Post => Color::Yellow,
        Method::Put => Color::Blue,
        Method::Patch => Color::Magenta,
        Method::Delete => Color::Red,
        Method::Head | Method::Options => Color::Cyan,
    };
    Style::default().fg(color)
}

fn centered(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(vertical[1])[1]
}

fn highlight_json(line: &str) -> Line<'_> {
    let bytes = line.as_bytes();
    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut index = 0;

    while index < bytes.len() {
        let token_start = index;
        let style = match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' => index = (index + 2).min(bytes.len()),
                        b'"' => {
                            index += 1;
                            break;
                        }
                        _ => index += 1,
                    }
                }
                let mut following = index;
                while following < bytes.len() && bytes[following].is_ascii_whitespace() {
                    following += 1;
                }
                if following < bytes.len() && bytes[following] == b':' {
                    Style::default().fg(ACCENT)
                } else {
                    Style::default().fg(Color::Green)
                }
            }
            b'-' | b'0'..=b'9' => {
                index += 1;
                while index < bytes.len()
                    && matches!(bytes[index], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
                {
                    index += 1;
                }
                Style::default().fg(Color::Yellow)
            }
            b't' if bytes[index..].starts_with(b"true") => {
                index += 4;
                Style::default().fg(Color::Magenta)
            }
            b'f' if bytes[index..].starts_with(b"false") => {
                index += 5;
                Style::default().fg(Color::Magenta)
            }
            b'n' if bytes[index..].starts_with(b"null") => {
                index += 4;
                Style::default().fg(MUTED)
            }
            _ => {
                index += 1;
                continue;
            }
        };

        if token_start > plain_start {
            spans.push(Span::raw(&line[plain_start..token_start]));
        }
        spans.push(Span::styled(&line[token_start..index], style));
        plain_start = index;
    }

    if plain_start < line.len() {
        spans.push(Span::raw(&line[plain_start..]));
    }
    if spans.is_empty() {
        spans.push(Span::raw(line));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};
    use tempfile::tempdir;

    use super::*;
    use crate::storage::Storage;

    #[test]
    fn highlights_json_keys_strings_and_literals() {
        let line = highlight_json("  \"ok\": true, \"value\": \"yes\"");
        assert_eq!(line.spans.len(), 8);
        assert_eq!(line.spans[1].style.fg, Some(ACCENT));
        assert_eq!(line.spans[3].style.fg, Some(Color::Magenta));
        assert_eq!(line.spans[7].style.fg, Some(Color::Green));
    }

    #[test]
    fn renders_main_screen_and_workspace_overlay() {
        let directory = tempdir().unwrap();
        let storage = Storage::new(directory.path().join("workspaces.json"));
        let mut app = App::new(storage).unwrap();
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| draw(frame, &app)).unwrap();
        app.overlay = Some(Overlay::Workspaces);
        terminal.draw(|frame| draw(frame, &app)).unwrap();

        assert_eq!(terminal.backend().buffer().area.width, 120);
        assert_eq!(terminal.backend().buffer().area.height, 40);
    }
}
