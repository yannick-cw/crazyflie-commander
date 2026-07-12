use crate::model::HomeState;
use crate::model::ModeSelection::{FreeFlightItem, MissionPlanItem, MissionSelectItem};
use crate::view::view_common::theme::*;
use crate::view::view_common::{controls, panel, selectable, shell};

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn view(model: &HomeState, frame: &mut Frame) {
    let area = frame.area();

    let shell = shell(controls(&[
        ("j/k ↑↓", "navigate", BRAND),
        ("↵", "select", SELECTED),
        ("q", "quit", LABEL),
    ]));
    let inner = shell.inner(area);
    frame.render_widget(shell, area);

    // centre a modest menu panel
    let [_, mid, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(9),
        Constraint::Fill(1),
    ])
    .areas(inner);
    let [_, menu_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(48),
        Constraint::Fill(1),
    ])
    .areas(mid);

    let modes = [
        (MissionSelectItem, "Select Mission"),
        (MissionPlanItem, "Plan Mission"),
        (FreeFlightItem, "Free Flight"),
    ];

    let mut lines: Vec<Line> = modes
        .iter()
        .map(|&(mode, name)| selectable(name, mode == model.selected_mode))
        .collect();

    // one-line description of the highlighted mode
    let description = match model.selected_mode {
        MissionSelectItem => "pick a saved mission and fly it",
        MissionPlanItem => "build a route from waypoints", // TODO: not built yet
        FreeFlightItem => "manual, observe-only flight",   // TODO: observe only for now
    };
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("   {description}"),
        Style::new().fg(LABEL).add_modifier(Modifier::ITALIC),
    )));

    frame.render_widget(Paragraph::new(lines).block(panel(" MODE ")), menu_area);
}
