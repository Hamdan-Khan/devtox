use crate::app::{App, PanelFocus};
use crossterm::style;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

// basically 1. splits the frame into rects (i.e. sections) and
// 2. renders ui stuff on those frame sections mostly based on the app state (reason for passing app ref)
pub fn draw(frame: &mut Frame, app: &App) {
    // split entire layout horizontally into two parts: left panels section and main section
    // giving left panels a fixed width for now, can implement something like media queries
    // (if possible ofc) later todo
    let [sidebar, main] =
        Layout::horizontal([Constraint::Length(24), Constraint::Fill(1)]).areas(frame.area());

    // split left section vertically: language panel (top) and artifact panel (bottom)
    let [lang_area, artifact_area] =
        Layout::vertical([Constraint::Percentage(40), Constraint::Fill(1)]).areas(sidebar);

    // split main section vertically: results (top) and stats bar (bottom)
    let [results_area, stats_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(main);

    render_languages(frame, app, lang_area);
    render_artifacts(frame, app, artifact_area);
    render_results_placeholder(frame, app, results_area);
    render_stats(frame, stats_area);
}

fn render_languages(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == PanelFocus::Languages;

    let items: Vec<ListItem> = app
        .language_list
        .items
        .iter() // !todo: understand iterators in depth (looks kind of like js arry functions)
        .map(|lang| ListItem::new(lang.display_name()))
        .collect();

    let list = List::new(items)
        .block(styled_block("Languages", focused))
        .highlight_symbol("▶ ")
        .highlight_style(highlight_style(focused));

    let mut state = ListState::default();
    state.select(Some(app.language_list.selected));

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_artifacts(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == PanelFocus::Artifacts;

    let title = app
        .language_list
        .selected_item()
        .map(|l| format!("{} artifacts", l.display_name()))
        .unwrap_or_else(|| "Artifacts".to_string());

    let items: Vec<ListItem> = app
        .artifact_list
        .items
        .iter()
        .map(|a| ListItem::new(a.display_name()))
        .collect();

    let list = List::new(items)
        .block(styled_block(&title, focused))
        .highlight_symbol("▶ ")
        .highlight_style(highlight_style(focused));

    let mut state = ListState::default();
    state.select(Some(app.artifact_list.selected));

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_results_placeholder(frame: &mut Frame, app: &App, area: Rect) {
    let description = match (
        app.language_list.selected_item(),
        app.artifact_list.selected_item(),
    ) {
        (Some(lang), Some(artifact)) => format!(
            "Ready to scan for '{}' ({}) directories.\nPress <s> to start scan.",
            artifact.display_name(),
            lang.display_name()
        ),
        _ => "Select a language and artifact type to scan.".to_string(),
    };

    let paragraph = Paragraph::new(description)
        .block(styled_block("Results", false))
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}

// stats to show at the bottom, when scanned (stubs for now)
fn render_stats(frame: &mut Frame, area: Rect) {
    let stats_line = Line::from(vec![
        Span::styled(" Total found: ", Style::default().fg(Color::Gray)),
        Span::styled(
            "—",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   Reclaimable: ", Style::default().fg(Color::Gray)),
        Span::styled(
            "—",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   Selected: ", Style::default().fg(Color::Gray)),
        Span::styled(
            "—",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   [q] quit  [tab] switch panel  [s] scan",
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let paragraph = Paragraph::new(stats_line).block(Block::default().borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

// styled panel / section block with title and conditional color based on the focused state
fn styled_block(title: &'_ str, focused: bool) -> Block<'_> {
    let mut title_style = Style::default();
    let mut border_style = Style::default();

    if focused {
        border_style = border_style.fg(Color::Green);
        title_style = title_style.fg(Color::Green).add_modifier(Modifier::BOLD);
    } else {
        border_style = border_style.fg(Color::DarkGray);
        title_style = title_style.fg(Color::DarkGray);
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(format!(" {title} "), title_style))
}

// selected list item style, gray/white when the panel is not focused, colorful otherwise
fn highlight_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .bg(Color::Green)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    }
}
