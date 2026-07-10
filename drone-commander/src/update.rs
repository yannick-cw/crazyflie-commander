use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::app::Model;
use crate::event::Message;

pub fn update(model: &Model, msg: Message) -> (Model, Option<Message>) {
    let mut model: Model = model.clone();
    match msg {
        Message::Tick => (model, None),
        Message::Key(key_event) => match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => (model, Some(Message::Quit)),
            KeyCode::Char('c') | KeyCode::Char('C')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                (model, Some(Message::Quit))
            }
            KeyCode::Right | KeyCode::Char('j') => (model, Some(Message::Increment)),
            KeyCode::Left | KeyCode::Char('k') => (model, Some(Message::Decrement)),
            _ => (model, None),
        },
        Message::Increment => {
            model.counter -= 1;
            (model, None)
        }
        Message::Decrement => {
            model.counter += 1;
            (model, None)
        }
        Message::Quit => {
            model.exit = true;
            (model, None)
        }
    }
}
