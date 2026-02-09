use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Mode, Tab, VisibleItem};
use crate::systemd::ChangeAction;

pub fn render(frame: &mut Frame, app: &App) {
    let [header_area, list_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(frame, app, header_area);
    render_service_list(frame, app, list_area);
    render_status_bar(frame, app, status_area);

    match app.mode {
        Mode::Confirm => render_confirm_modal(frame, app),
        Mode::Applying => render_applying_overlay(frame),
        _ => {}
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let system_style = if app.tab == Tab::System {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let user_style = if app.tab == Tab::User {
        Style::default().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let header = Line::from(vec![
        Span::raw(" "),
        Span::styled(" System ", system_style),
        Span::raw("  "),
        Span::styled(" User ", user_style),
        Span::raw("          Tab: switch  /: search  q: quit"),
    ]);

    frame.render_widget(Paragraph::new(header), area);
}

fn render_service_list(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::TOP);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Calculate scroll offset to keep cursor visible
    let max_visible = inner.height as usize;
    let scroll_offset = if app.cursor >= max_visible {
        app.cursor - max_visible + 1
    } else {
        0
    };

    let mut lines: Vec<Line> = Vec::new();

    for (idx, item) in app
        .visible_items
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(max_visible)
    {
        let is_cursor = idx == app.cursor;

        let line = match item {
            VisibleItem::Category(cat_idx) => {
                let cat = &app.categories[*cat_idx];
                let arrow = if cat.collapsed { "▸" } else { "▾" };
                let count = cat.services.len();
                let style = if is_cursor {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                let cursor_indicator = if is_cursor { ">" } else { " " };
                Line::from(vec![
                    Span::styled(
                        format!("{cursor_indicator} {arrow} {}", cat.name),
                        style,
                    ),
                    Span::styled(format!(" ({count})"), Style::default().fg(Color::DarkGray)),
                ])
            }
            VisibleItem::Service(svc_idx) => {
                let svc = &app.services[*svc_idx];
                let checkbox = if svc.enabled { "[✓]" } else { "[ ]" };
                let dirty = app.is_service_dirty(svc);

                let style = if is_cursor && dirty {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else if is_cursor {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else if dirty {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };

                let cursor_indicator = if is_cursor { ">" } else { " " };
                Line::from(Span::styled(
                    format!("{cursor_indicator}   {checkbox} {}", svc.name),
                    style,
                ))
            }
        };

        lines.push(line);
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let line = match app.mode {
        Mode::Filter => {
            Line::from(vec![
                Span::styled(" /: ", Style::default().fg(Color::Cyan)),
                Span::raw(&app.filter),
                Span::styled("▏", Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled("[Enter] Keep", Style::default().fg(Color::Green)),
                Span::raw("  "),
                Span::styled("[Esc] Clear", Style::default().fg(Color::DarkGray)),
            ])
        }
        _ => {
            let mut spans = Vec::new();
            if !app.filter.is_empty() {
                spans.push(Span::styled(
                    format!(" filter: {}", app.filter),
                    Style::default().fg(Color::Cyan),
                ));
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[Esc] Clear",
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::raw("  "));
            }
            let count = app.pending_count();
            if count > 0 {
                spans.push(Span::styled(
                    format!(" {count} pending change{}", if count == 1 { "" } else { "s" }),
                    Style::default().fg(Color::Yellow),
                ));
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[Enter] Apply",
                    Style::default().fg(Color::Green),
                ));
            } else if !app.results.is_empty() {
                let success = app.results.iter().filter(|r| r.success).count();
                let failed = app.results.iter().filter(|r| !r.success).count();
                if failed == 0 {
                    spans.push(Span::styled(
                        format!(" ✓ {success} applied"),
                        Style::default().fg(Color::Green),
                    ));
                } else {
                    spans.push(Span::styled(
                        format!(" ✓ {success} applied, ✗ {failed} failed"),
                        Style::default().fg(Color::Red),
                    ));
                }
            } else {
                spans.push(Span::styled(
                    " Space: toggle  Enter: apply  q: quit",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Line::from(spans)
        }
    };

    frame.render_widget(Paragraph::new(line), area);
}

fn render_applying_overlay(frame: &mut Frame) {
    let area = frame.area();
    let w = 30u16.min(area.width.saturating_sub(4));
    let h = 3u16;
    let modal = Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let text = Paragraph::new(Line::styled(
        " Applying changes...",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ))
    .block(block);
    frame.render_widget(text, modal);
}

fn render_confirm_modal(frame: &mut Frame, app: &App) {
    let changes = app.pending_changes();
    if changes.is_empty() {
        return;
    }

    let area = frame.area();
    let modal_width = 50u16.min(area.width.saturating_sub(4));
    let modal_height = (changes.len() as u16 + 7).min(area.height.saturating_sub(4));
    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, modal_area);

    let mut lines = vec![
        Line::raw(""),
        Line::styled(
            " The following changes will be applied:",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];

    for change in &changes {
        let (icon, action_text) = match change.action {
            ChangeAction::Enable => ("●", "Enable + Start"),
            ChangeAction::Disable => ("●", "Disable + Stop"),
        };
        let color = match change.action {
            ChangeAction::Enable => Color::Green,
            ChangeAction::Disable => Color::Red,
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, Style::default().fg(color)),
            Span::raw(format!(" {action_text}  {}", change.service)),
        ]));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled(" [Enter] Confirm", Style::default().fg(Color::Green)),
        Span::raw("    "),
        Span::styled("[Esc] Cancel", Style::default().fg(Color::DarkGray)),
    ]));

    let block = Block::default()
        .title(" Apply Changes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, modal_area);
}
