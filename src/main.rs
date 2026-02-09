mod app;
mod categories;
mod systemd;
mod tui;

use std::time::Duration;

use anyhow::Result;
use crossterm::event;
use tokio::sync::oneshot;

use app::{App, Mode};
use systemd::{apply_changes, ChangeResult};
use tui::handler::{handle_event, Action};
use tui::ui::render;

#[tokio::main]
async fn main() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal).await;
    ratatui::restore();
    result
}

async fn run(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = App::new()?;
    let mut pending_apply: Option<oneshot::Receiver<Vec<ChangeResult>>> = None;

    loop {
        terminal.draw(|frame| render(frame, &app))?;

        // Check if background apply has completed
        if let Some(ref mut rx) = pending_apply {
            match rx.try_recv() {
                Ok(results) => {
                    let _ = app.apply_done(results);
                    app.mode = Mode::Normal;
                    pending_apply = None;
                }
                Err(oneshot::error::TryRecvError::Empty) => {
                    // Still running, keep spinning
                }
                Err(oneshot::error::TryRecvError::Closed) => {
                    // Task panicked or was dropped
                    app.mode = Mode::Normal;
                    pending_apply = None;
                }
            }
        }

        if event::poll(Duration::from_millis(50))? {
            let action = handle_event(&mut app, event::read()?);

            if let Action::ApplyChanges = action {
                let changes = app.pending_changes();
                app.mode = Mode::Applying;

                let (tx, rx) = oneshot::channel();
                pending_apply = Some(rx);

                tokio::spawn(async move {
                    let results = apply_changes(changes).await;
                    let _ = tx.send(results);
                });
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
