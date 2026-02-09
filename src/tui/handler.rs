use crossterm::event::{Event, KeyCode, KeyEventKind};

use crate::app::{App, Mode};

/// What the main loop should do after handling an event.
pub enum Action {
    None,
    ApplyChanges,
}

pub fn handle_event(app: &mut App, event: Event) -> Action {
    if let Event::Key(key) = event {
        if key.kind != KeyEventKind::Press {
            return Action::None;
        }

        return match app.mode {
            Mode::Normal => handle_normal(app, key.code),
            Mode::Filter => handle_filter(app, key.code),
            Mode::Confirm => handle_confirm(app, key.code),
            Mode::Applying => Action::None, // ignore input while applying
            Mode::Info => handle_info(app, key.code),
        };
    }
    Action::None
}

fn handle_normal(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => app.move_cursor(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_cursor(1),
        KeyCode::Char(' ') => app.toggle_current(),
        KeyCode::Enter => {
            if app.has_pending_changes() {
                app.mode = Mode::Confirm;
            }
        }
        KeyCode::Tab => {
            let _ = app.switch_tab();
        }
        KeyCode::Left | KeyCode::Char('h') => app.toggle_collapse(),
        KeyCode::Right | KeyCode::Char('l') => app.toggle_collapse(),
        KeyCode::Esc => {
            if !app.filter.is_empty() {
                app.filter.clear();
                app.rebuild_visible();
                app.cursor = 0;
            }
        }
        KeyCode::Char('i') => app.show_info(),
        KeyCode::Char('/') => {
            app.mode = Mode::Filter;
            app.filter.clear();
        }
        _ => {}
    }
    Action::None
}

fn handle_filter(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
            app.filter.clear();
            app.rebuild_visible();
            app.cursor = 0;
        }
        KeyCode::Enter => {
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            app.filter.pop();
            app.rebuild_visible();
            app.cursor = 0;
        }
        KeyCode::Up => app.move_cursor(-1),
        KeyCode::Down => app.move_cursor(1),
        KeyCode::Char(c) => {
            app.filter.push(c);
            app.rebuild_visible();
            app.cursor = 0;
        }
        _ => {}
    }
    Action::None
}

fn handle_info(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('q') => {
            app.mode = Mode::Normal;
            app.info = None;
        }
        _ => {}
    }
    Action::None
}

fn handle_confirm(app: &mut App, code: KeyCode) -> Action {
    match code {
        KeyCode::Enter => {
            app.mode = Mode::Normal;
            return Action::ApplyChanges;
        }
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
    Action::None
}
