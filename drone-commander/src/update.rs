use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::event::Message;
use crate::model::HomeState::Overview;
use crate::model::{Model, State};

pub fn update(model: &Model, msg: Message) -> (Model, Option<Message>) {
    let mut model: Model = model.clone();
    match msg {
        Message::Tick(tele) => {
            model.telemetry = tele;
            (model, None)
        }
        Message::Key(key_event) => match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => (model, Some(Message::Quit)),
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                (model, Some(Message::Quit))
            }
            KeyCode::Char('j') | KeyCode::Down => (model, Some(Message::Down)),
            KeyCode::Char('k') | KeyCode::Up => (model, Some(Message::Up)),
            _ => (model, None),
        },
        Message::Quit => {
            model.exit = true;
            (model, None)
        }
        other_msg => match model.state {
            State::Home(Overview(current_selection)) => match other_msg {
                Message::Up => {
                    model.state = State::Home(Overview(current_selection.prev()));
                    (model, None)
                }
                Message::Down => {
                    model.state = State::Home(Overview(current_selection.next()));
                    (model, None)
                }
                _ => (model, None),
            },
            _ => (model, None),
        },
    }
}
