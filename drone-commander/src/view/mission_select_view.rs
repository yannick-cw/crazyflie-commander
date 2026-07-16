use crate::pages::mission_select::Model;
use crate::view::view_common::theme::*;
use crate::view::view_common::{controls, panel, selectable, shell};
use drone_control::Command;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

// AI GENERATED

pub fn view(model: &Model, frame: &mut Frame) {
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

    // two sections, but one flat selection index running across both
    let lines: Vec<Line> = section("missions", &model.missions, 0, model.selection)
        .into_iter()
        .chain(std::iter::once(Line::from("")))
        .chain(section(
            "recordings",
            &model.recorded_missions,
            model.missions.len(),
            model.selection,
        ))
        .collect();

    frame.render_widget(Paragraph::new(lines).block(panel(" LIBRARY ")), list_area);
    frame.render_widget(details(model), detail_area);
}

/// One titled section: a header, then each mission as a selectable row (or a dim
/// placeholder when empty). `offset` is where this section starts in the flat index.
fn section(
    title: &str,
    missions: &[(String, Vec<Command>)],
    offset: usize,
    selection: usize,
) -> Vec<Line<'static>> {
    let header = Line::from(Span::styled(
        format!(" {} ", title.to_uppercase()),
        Style::new().fg(TITLE).add_modifier(Modifier::BOLD),
    ));
    let rows: Vec<Line> = if missions.is_empty() {
        vec![Line::from(Span::styled(
            "   (none yet)",
            Style::new().fg(LABEL).add_modifier(Modifier::ITALIC),
        ))]
    } else {
        missions
            .iter()
            .enumerate()
            .map(|(i, (name, cmds))| {
                let mut row = selectable(name, offset + i == selection);
                row.spans.push(Span::styled(
                    format!("   ({} steps)", cmds.len()),
                    Style::new().fg(LABEL),
                ));
                row
            })
            .collect()
    };
    std::iter::once(header).chain(rows).collect()
}

fn details(model: &Model) -> Paragraph<'static> {
    let selected = model
        .missions
        .iter()
        .chain(model.recorded_missions.iter())
        .nth(model.selection);
    let content = match selected {
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
        None => vec![Line::from(Span::styled("no missions", Style::new().fg(LABEL)))],
    };
    Paragraph::new(content).block(panel(" DETAILS "))
}

fn field(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<10}"), Style::new().fg(LABEL)),
        Span::styled(value, Style::new().fg(TITLE).add_modifier(Modifier::BOLD)),
    ])
}
