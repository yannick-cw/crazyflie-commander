use crate::model::HomeState;
use crate::model::ModeSelection::{FreeFlightItem, MissionPlanItem, MissionSelectItem};
use crate::view::view_common::theme::*;
use crate::view::view_common::{center, controls, panel, selectable, shell};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

// AI GENERATED

pub fn view(model: &HomeState, frame: &mut Frame) {
    let area = frame.area();

    let shell = shell(controls(&[
        ("j/k ↑↓", "navigate", BRAND),
        ("↵", "select", SELECTED),
        ("q", "quit", LABEL),
    ]));
    let inner = shell.inner(area);
    frame.render_widget(shell, area);

    // menu fills, with a warning banner pinned to the bottom
    let [body, warning_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).areas(inner);
    frame.render_widget(warning(), warning_area);

    // centre a modest menu panel
    let menu_area = center(body, 48, 9);

    let modes = [
        (MissionSelectItem, "Select Mission"),
        (MissionPlanItem, "Plan Mission"),
        (FreeFlightItem, "Free Flight"),
    ];

    let mut lines: Vec<Line> = modes
        .iter()
        .map(|&(ref mode, name)| selectable(name, *mode == model.selected_mode))
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

/// Loud reminder: the position estimate only resets on a fresh start, so moving the
/// drone by hand between flights corrupts it.
fn warning() -> Paragraph<'static> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(DANGER))
        .title(Span::styled(
            " ⚠ WARNING ",
            Style::new().fg(DANGER).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center);
    Paragraph::new(vec![
        Line::from(Span::styled(
            "DO NOT move the Crazyflie by hand between flights",
            Style::new().fg(DANGER).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "it corrupts the position estimate (only reset on restart)",
            Style::new().fg(WARN),
        )),
        Line::from(Span::styled(
            "Flow deck required · position & heading are measured from takeoff",
            Style::new().fg(WARN),
        )),
    ])
    .alignment(Alignment::Center)
    .block(block)
}
