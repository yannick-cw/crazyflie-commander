use crate::model::MissionSelectState;
use crate::view::view_common::theme::*;
use crate::view::view_common::{controls, panel, selectable, shell};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn view(model: &MissionSelectState, frame: &mut Frame) {
    let area = frame.area();

    let shell = shell(controls(&[
        ("j/k ↑↓", "navigate", BRAND),
        ("↵", "fly", SELECTED),
        ("b", "back", WARN),
        ("q", "quit", LABEL),
    ]));
    let inner = shell.inner(area);
    frame.render_widget(shell, area);

    // mission list (fills) · details of the selected one (fixed width)
    let [list_area, detail_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(32)]).areas(inner);

    let items: Vec<Line> = model
        .missions
        .iter()
        .enumerate()
        .map(|(i, (name, cmds))| {
            let mut row = selectable(name, i == model.selection);
            row.spans.push(Span::styled(
                format!("   ({} steps)", cmds.len()),
                Style::new().fg(LABEL),
            ));
            row
        })
        .collect();

    frame.render_widget(Paragraph::new(items).block(panel(" MISSIONS ")), list_area);
    frame.render_widget(details(model), detail_area);
}

fn details(model: &MissionSelectState) -> Paragraph<'static> {
    let content = match model.missions.get(model.selection) {
        Some((name, cmds)) => vec![
            Line::from(Span::styled(
                name.clone(),
                Style::new().fg(BRAND).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            field("steps", cmds.len().to_string()),
            field("est. time", "~1m 30s".to_string()), // TODO: real estimate later
            Line::from(""),
            Line::from(Span::styled(
                "a saved flight pattern", // TODO: real description later
                Style::new().fg(LABEL).add_modifier(Modifier::ITALIC),
            )),
        ],
        None => vec![Line::from(Span::styled(
            "no missions",
            Style::new().fg(LABEL),
        ))],
    };
    Paragraph::new(content).block(panel(" DETAILS "))
}

fn field(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<10}"), Style::new().fg(LABEL)),
        Span::styled(value, Style::new().fg(TITLE).add_modifier(Modifier::BOLD)),
    ])
}
