use crate::{
    app::{App, PanelFocus, ScanState},
    utils::format_size_str,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Padding, Paragraph},
};

// basically 1. splits the frame into rects (i.e. sections) and
// 2. renders ui stuff on those frame sections mostly based on the app state (reason for passing app ref)
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // split entire layout horizontally into two parts: left panels section and main section
    // giving left panels a fixed width for now, can implement something like media queries
    // (if possible ofc) later todo
    let [sidebar, main] =
        Layout::horizontal([Constraint::Length(24), Constraint::Fill(1)]).areas(area);

    // split left section vertically: language panel (top) and artifact panel (bottom)
    let [lang_area, artifact_area] =
        Layout::vertical([Constraint::Percentage(40), Constraint::Fill(1)]).areas(sidebar);

    // split main section vertically: results (top) and stats bar (bottom)
    let [results_area, stats_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(main);

    render_languages(frame, app, lang_area);
    render_artifacts(frame, app, artifact_area);
    render_scan_screen(frame, app, results_area);
    render_stats(frame, app, stats_area);
    render_input_modal(frame, app, area);
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

fn keybind_line(key: &str, desc: &str) -> String {
    format!("{:<7} {}", key, desc)
}

fn render_scan_screen(frame: &mut Frame, app: &App, area: Rect) {
    const SPINNER_STATES: &[&str] = &["⣷", "⣯", "⣟", "⡿", "⢿", "⣻", "⣽", "⣾"];

    let description = match (
        app.language_list.selected_item(),
        app.artifact_list.selected_item(),
    ) {
        (Some(lang), Some(artifact)) => match app.scan_state {
            ScanState::Idle => format!(
                "Ready to scan for '{}' ({}) directories.\n\nPress\n{}\n{}\n{}\n{}\n{}",
                artifact.display_name(),
                lang.display_name(),
                keybind_line("<Enter>", "select language / artifact"),
                keybind_line("<s>", "start scan"),
                keybind_line("<Tab>", "switch selection panels"),
                keybind_line("<Esc>", "go back to selection"),
                keybind_line("<q>", "quit"),
            ),
            ScanState::Confirmation => {
                format!(
                    "Are you sure you want to scan for all the '{}' directories in '{}'?\n\nPress\n{}\n{}",
                    artifact.display_name(),
                    app.selected_entry_dir,
                    keybind_line("<y>", "to proceed"),
                    keybind_line("<n>", "to abort"),
                )
            }
            ScanState::InProgress => {
                let state_index = ((app.tick / 6) as usize) % SPINNER_STATES.len();
                let spinner = SPINNER_STATES[state_index];
                format!("Scanning '{}' {}", app.selected_entry_dir, spinner)
            }
            ScanState::Completed(_) => {
                format!(
                    "Successfully scanned '{}' for '{}'!\n\nPress\n{}\n{}",
                    app.selected_entry_dir,
                    artifact.display_name(),
                    keybind_line("<Esc>", "go back to selection and start a new scan session"),
                    keybind_line("<q>", "quit"),
                )
            }
            ScanState::Error => String::from("Couldn't scan due to an error."),
        },
        _ => "Select a language and artifact type to scan.".to_string(),
    };

    let focused = app.focus == PanelFocus::Results;
    let text_color = if focused {
        Color::White
    } else {
        Color::DarkGray
    };
    let paragraph = Paragraph::new(description)
        .block(styled_block("Results", focused))
        .style(Style::default().fg(text_color));

    frame.render_widget(paragraph, area);
}

// stats to show at the bottom, when scanned
fn render_stats(frame: &mut Frame, app: &App, area: Rect) {
    let total_size = format_size_str(app.scan_result.total_size as f64);

    let stats_line = Line::from(vec![
        Span::styled(" Total found: ", Style::default().fg(Color::Gray)),
        Span::styled(
            total_size,
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
    ]);

    let paragraph = Paragraph::new(stats_line).block(Block::default().borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn render_input_modal(frame: &mut Frame, app: &App, area: Rect) {
    if app.show_input_modal {
        let focused = app.focus == PanelFocus::InputModal;
        let popup_block = styled_block("Create custom search artifact", focused);
        let centered_area = area.centered(Constraint::Percentage(60), Constraint::Percentage(40));
        // clears out any background in the area before rendering the popup
        frame.render_widget(Clear, centered_area);
        let paragraph = Paragraph::new("todo: input")
            .block(popup_block)
            .style(Style::default().bg(Color::Rgb(46, 46, 46)));
        frame.render_widget(paragraph, centered_area);
    }
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
        .padding(Padding::horizontal(1))
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
