use std::collections::HashSet;

use crate::{
    app::{App, PanelFocus, ScanResult, ScanState},
    utils::format_size_str,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph,
        Row, Table,
    },
};

// basically 1. splits the frame into rects (i.e. sections) and
// 2. renders ui stuff on those frame sections mostly based on the app state (reason for passing app ref)
pub fn draw(frame: &mut Frame, app: &mut App) {
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

fn render_scan_screen(frame: &mut Frame, app: &mut App, area: Rect) {
    const SPINNER_STATES: &[&str] = &["⣷", "⣯", "⣟", "⡿", "⢿", "⣻", "⣽", "⣾"];

    let focused = app.focus == PanelFocus::Results;
    let text_color = if focused {
        Color::White
    } else {
        Color::DarkGray
    };
    let table_state = &mut app.table_state;

    match (
        app.language_list.selected_item(),
        app.artifact_list.selected_item(),
    ) {
        (Some(lang), Some(artifact)) => match &app.scan_state {
            ScanState::Completed(scan_result) => {
                let block = styled_block("Results", focused);
                let inner = block.inner(area);
                frame.render_widget(block, area);

                let sections = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // search bar
                        Constraint::Length(1), // actions row
                        Constraint::Fill(1),   // table
                    ])
                    .split(inner);

                let search_style = if app.search_focused {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let search_prefix = if app.search_focused { " / " } else { " " };
                let search_display = if app.search_query.is_empty() && !app.search_focused {
                    format!("{}[Press 's' to search paths]", search_prefix)
                } else {
                    format!("{}{}▌", search_prefix, app.search_query)
                };
                let search_bar = Paragraph::new(search_display).style(search_style);
                frame.render_widget(search_bar, sections[0]);
                let total = scan_result.scanned_entries.len();
                let selected_count = app.selected_entries.len();
                let actions = Paragraph::new(format!(
                    " [a] Select all  [d] Deselect all  │  {}/{} selected",
                    selected_count, total
                ))
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(actions, sections[1]);

                let table = generate_scanned_table(
                    scan_result,
                    focused,
                    &app.search_query,
                    &app.selected_entries,
                );
                frame.render_stateful_widget(table, sections[2], table_state);
            }
            _ => {
                let description = match &app.scan_state {
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
                    ScanState::Confirmation => format!(
                        "Are you sure you want to scan for all the '{}' directories in '{}'?\n\nPress\n{}\n{}",
                        artifact.display_name(),
                        app.selected_entry_dir,
                        keybind_line("<y>", "to proceed"),
                        keybind_line("<n>", "to abort"),
                    ),
                    ScanState::InProgress => {
                        let state_index = ((app.tick / 6) as usize) % SPINNER_STATES.len();
                        format!(
                            "Scanning '{}' {}",
                            app.selected_entry_dir, SPINNER_STATES[state_index]
                        )
                    }
                    ScanState::Error => String::from("Couldn't scan due to an error."),
                    ScanState::Completed(_) => unreachable!(),
                };

                let paragraph = Paragraph::new(description)
                    .block(styled_block("Results", focused))
                    .style(Style::default().fg(text_color));

                frame.render_widget(paragraph, area);
            }
        },
        _ => {
            let paragraph = Paragraph::new("Select a language and artifact type to scan.")
                .block(styled_block("Results", focused))
                .style(Style::default().fg(text_color));

            frame.render_widget(paragraph, area);
        }
    }
}

fn generate_scanned_table<'a>(
    scan_result: &'a ScanResult,
    focused: bool,
    search_query: &str,
    selected_entries: &HashSet<usize>,
) -> Table<'a> {
    let text_color = if focused {
        Color::White
    } else {
        Color::DarkGray
    };
    let query = search_query.to_lowercase();

    let header = Row::new(vec![
        Cell::from("✓").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("#").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Path").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Size").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(text_color))
    .bottom_margin(1);

    let rows: Vec<Row> = scan_result
        .scanned_entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| query.is_empty() || entry.path.to_lowercase().contains(&query))
        .map(|(i, entry)| {
            let size = format_size_str(entry.size as f64);
            let is_selected = selected_entries.contains(&i);
            let check_cell = if is_selected {
                Cell::from("✓").style(
                    Style::default()
                        .fg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Cell::from("○").style(Style::default().fg(Color::DarkGray))
            };
            Row::new(vec![
                check_cell,
                Cell::from(format!("{}", i + 1)),
                Cell::from(entry.path.clone()),
                Cell::from(size),
            ])
            .style(Style::default().fg(text_color))
        })
        .collect();

    Table::new(
        rows,
        [
            Constraint::Length(3),  // select column
            Constraint::Length(4),  // #
            Constraint::Fill(1),    // path
            Constraint::Length(10), // size
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::horizontal(1)),
    )
    .column_spacing(1)
    .style(Color::White)
    .row_highlight_style(Style::new().on_light_green().bold())
    .column_highlight_style(Color::Gray)
}

// stats to show at the bottom, when scanned
fn render_stats(frame: &mut Frame, app: &App, area: Rect) {
    let total_size = match &app.scan_state {
        ScanState::Completed(scan_result) => format_size_str(scan_result.total_size as f64),
        _ => String::from("0"),
    };

    let stats_line = Line::from(vec![
        Span::styled(" Total found: ", Style::default().fg(Color::Gray)),
        Span::styled(
            total_size,
            Style::default()
                .fg(Color::White)
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
