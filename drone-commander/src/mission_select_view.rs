use crate::flight_view::{shell, theme};
use crate::model::MissionSelectState;
use ratatui::widgets::{List, ListItem};
use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
};

pub fn view(model: &MissionSelectState, frame: &mut Frame) {
    let area = frame.area();

    let shell = shell();
    let inner = shell.inner(area);
    frame.render_widget(shell, area);

    let list_items: Vec<ListItem> = model
        .missions
        .iter()
        .enumerate()
        .map(|(i, (name, _))| list_item(name, i == model.selection))
        .collect();

    let list = List::new(list_items);

    frame.render_widget(list, inner);
}

fn list_item(content: &str, selected: bool) -> ListItem<'_> {
    ListItem::new(Line::from(Span::styled(
        content,
        Style::default().fg(if selected {
            theme::SELECTED
        } else {
            theme::LABEL
        }),
    )))
}
