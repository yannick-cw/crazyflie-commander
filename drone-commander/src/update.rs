use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub fn update(app: &mut App, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => app.exit(),
        KeyCode::Char('c') | KeyCode::Char('C') if key_event.modifiers == KeyModifiers::CONTROL => {
            app.exit()
        }
        KeyCode::Right | KeyCode::Char('j') => app.increment(),
        KeyCode::Left | KeyCode::Char('k') => app.decrement(),
        _ => {}
    };
}
