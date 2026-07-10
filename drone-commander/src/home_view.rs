use crate::flight_view::{shell, theme};
use crate::model::ModeSelection;
use crate::model::ModeSelection::{FreeFlightItem, MissionPlanItem, MissionSelectItem};
use ratatui::widgets::{List, ListItem};
use ratatui::{
    Frame,
    style::Style,
    text::{Line, Span},
};

pub fn view(model: &ModeSelection, frame: &mut Frame) {
    let area = frame.area();

    let shell = shell();
    let inner = shell.inner(area);
    frame.render_widget(shell, area);

    let list_items = vec![
        list_item("Select Mission", *model == MissionSelectItem),
        list_item("Plan Mission", *model == MissionPlanItem),
        list_item("Free Flight", *model == FreeFlightItem),
    ];

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
