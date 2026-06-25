pub mod input;
pub mod state;

use std::collections::HashSet;

use crate::{
    app::App,
    model::scan::{DeleteState, ScanResult, ScanState},
    ui::state::PanelFocus,
    utils::{entry_matches_query, format_size_str},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph,
        Row, Table,
    },
};

const SPINNER_STATES: &[&str] = &["⣷", "⣯", "⣟", "⡿", "⢿", "⣻", "⣽", "⣾"];

// basically 1. splits the frame into rects (i.e. sections) and
// 2. renders ui stuff on those frame sections mostly based on the app state (reason for passing app ref)
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // split entire layout horizontally into two parts: left panels section and main section
    let [sidebar, main] =
        Layout::horizontal([Constraint::Percentage(15), Constraint::Fill(1)]).areas(area);

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
    render_artifact_input_modal(frame, app, area);
    render_deletion_modal(frame, app, area);
    render_path_input_modal(frame, app, area);
}

fn render_languages(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == PanelFocus::Languages;

    let items: Vec<ListItem> = app
        .language_list
        .items
        .iter()
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
    let focused = app.focus == PanelFocus::Results;
    let text_color = if focused {
        Color::White
    } else {
        Color::DarkGray
    };

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
                        Constraint::Length(3), // search bar
                        Constraint::Length(1), // actions row
                        Constraint::Fill(1),   // table
                    ])
                    .split(inner);

                render_search_input(frame, app, sections[0]);

                let total = scan_result.scanned_entries.len();
                let selected_count = app.selected_entries.len();
                let actions = Paragraph::new(format!(
                    " [Enter] toggle selection [d] Delete selected [a] Select all [x] Deselect all │ {}/{} selected",
                    selected_count, total
                ))
                .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(actions, sections[1]);

                let table = generate_scanned_table(
                    scan_result,
                    focused,
                    &app.search_input.query,
                    &app.selected_entries,
                );
                frame.render_stateful_widget(table, sections[2], &mut app.table_state);
            }
            _ => {
                let description : Text = match &app.scan_state {
                    ScanState::Idle => format!(
                        "Ready to scan for '{}' ({}) directories.\n\nPress\n{}\n{}\n{}\n{}\n{}\n{}",
                        artifact.display_name(),
                        lang.display_name(),
                        keybind_line("<Enter>", "select language / artifact"),
                        keybind_line("<p>", "set entry path to start the scan from"),
                        keybind_line("<s>", "start scan"),
                        keybind_line("<Tab>", "switch selection panels"),
                        keybind_line("<Esc>", "go back to selection"),
                        keybind_line("<q>", "quit"),
                    )
                    .into(),
                    ScanState::Confirmation => format!(
                        "Are you sure you want to scan for all the '{}' directories in '{}'?\n\nPress\n{}\n{}\n{}\n{}",
                        artifact.display_name(),
                        app.data.selected_entry_dir,
                        keybind_line("<p>", "set entry path to start the scan from"),
                        keybind_line("<y>", "to proceed"),
                        keybind_line("<n>", "to abort"),
                        keybind_line("<q>", "quit"),
                    ).into(),
                    ScanState::InProgress => {
                        // loading animation
                        let state_index = ((app.tick / 6) as usize) % SPINNER_STATES.len();
                        format!(
                            "Scanning '{}' {}",
                            app.data.selected_entry_dir, SPINNER_STATES[state_index]
                        ).into()
                    }
                    ScanState::Error(metadata) => Text::from(vec![
                        Line::from(vec![
                            Span::styled(
                                "Couldn't scan directory due to this error: ",
                                Style::default().fg(Color::Gray),
                            ),
                            Span::styled(
                                metadata.message.to_string(),
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(
                            vec![
                            Span::styled(
                                "Path selected: ",
                                Style::default().fg(Color::Gray),
                            ),
                            Span::styled(
                                format!("{:?}", metadata.path.as_deref().unwrap_or_default()),
                                Style::default().fg(Color::Red))
                            ]
                        ),
                    ]),
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

fn render_search_input(frame: &mut Frame, app: &App, area: Rect) {
    let search_style = if app.search_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let search_label: String = if app.search_focused {
        "Search (press 'Enter' to search)".to_string()
    } else {
        "Search (press 's')".to_string()
    };

    let search_display = if app.search_input.query.is_empty() && !app.search_focused {
        "[Press 's' to search paths]".to_string()
    } else {
        format!(" {}", app.search_input.query)
    };
    let search_bar = Paragraph::new(search_display)
        .style(search_style)
        .block(Block::bordered().title(search_label));
    frame.render_widget(search_bar, area);

    if app.search_focused {
        // Draw the cursor at the current position in the input field.
        // This position can be controlled via the left and right arrow key
        let x_pos: u16 = if app.search_input.query.is_empty() {
            // for empty search query, cursor should be at 0th column
            // hence the area's x + 1 as compared to when search query is written to,
            // we want the cursor one column ahead of the character that is written
            // hence area's x coordinate + cursor index + 2
            area.x + 1
        } else {
            area.x + app.search_input.char_index as u16 + 2
        };
        frame.set_cursor_position(Position::new(
            x_pos,
            // Move one line down, from the border to the input line
            area.y + 1,
        ));
    }
}

fn generate_scanned_table<'a>(
    scan_result: &'a ScanResult,
    focused: bool,
    search_query: &str,
    selected_entries: &HashSet<String>,
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
        .filter(|(_, entry)| entry_matches_query(entry, &query))
        .map(|(i, entry)| {
            let size = format_size_str(entry.size as f64);
            let is_selected = selected_entries.contains(&entry.path);
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
    let selected_size = match &app.scan_state {
        ScanState::Completed(_) => format_size_str(app.selected_size as f64),
        _ => String::from("0"),
    };
    let symlinks = match &app.scan_state {
        ScanState::Completed(scan_result) => scan_result.symlink_count.to_string(),
        _ => String::from("—"),
    };

    let stats_line = Line::from(vec![
        Span::styled(" Total found: ", Style::default().fg(Color::Gray)),
        Span::styled(
            total_size,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Selected: ", Style::default().fg(Color::Gray)),
        Span::styled(
            selected_size,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" SymLinks found: ", Style::default().fg(Color::Gray)),
        Span::styled(
            symlinks,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let paragraph = Paragraph::new(stats_line).block(Block::default().borders(Borders::ALL));

    frame.render_widget(paragraph, area);
}

fn render_deletion_modal(frame: &mut Frame, app: &App, area: Rect) {
    if app.delete_state != DeleteState::None {
        let popup_block = styled_block("Delete selected folders", true);
        let centered_area = area.centered(Constraint::Percentage(60), Constraint::Max(12));
        frame.render_widget(Clear, centered_area);

        // text area + button bar
        let layout = Layout::vertical([Constraint::Fill(1), Constraint::Length(3)])
            .split(popup_block.inner(centered_area));

        // outer block
        frame.render_widget(
            Block::default()
                .style(Style::default().bg(Color::Rgb(46, 46, 46)))
                .borders(Borders::ALL),
            centered_area,
        );
        frame.render_widget(popup_block, centered_area);

        let text = match &app.delete_state {
            DeleteState::Confirmation => Text::from(vec![
                // manually adding new line to all cases because formatting the final Text widget overrides the
                // styles applied to text
                Line::from("\n"),
                Line::from("Are you sure you want to delete the selected folders?"),
                Line::from("This action is destructive and cannot be undone."),
                Line::from(vec![
                    Span::raw("Please proceed with "),
                    Span::styled(
                        "caution",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("!"),
                ]),
            ]),
            DeleteState::InProgress => {
                // loading animation
                let state_index = ((app.tick / 6) as usize) % SPINNER_STATES.len();
                Text::from(format!(
                    "\nDeleting selected entries... {}",
                    SPINNER_STATES[state_index]
                ))
            }
            DeleteState::Completed(result) => {
                let total = result.len().to_string();

                Text::from(vec![
                    Line::from("\n"),
                    Line::from(vec![
                        Span::raw("Successfully deleted "),
                        Span::styled(
                            total,
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" entries!"),
                    ]),
                ])
            }
            DeleteState::None => Text::from("-"),
        };

        let paragraph = Paragraph::new(text)
            .style(Style::default().bg(Color::Rgb(46, 46, 46)))
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, layout[0]);

        // split button area into two halves
        let button_layout =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(layout[1]);

        match &app.delete_state {
            DeleteState::Completed(_) => {
                // using the full layout[1] area i.e. before splitting for the 2 buttons
                let continue_button = Paragraph::new("Continue [enter]")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green)),
                    )
                    .style(Style::default().fg(Color::Green));
                frame.render_widget(continue_button, layout[1]);
            }
            _ => {
                let yes_button = Paragraph::new("Yes, Proceed [y]")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Red)),
                    )
                    .style(Style::default().fg(Color::Red));

                let no_button = Paragraph::new("No, go back [n]")
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green)),
                    )
                    .style(Style::default().fg(Color::Green));

                frame.render_widget(yes_button, button_layout[0]);
                frame.render_widget(no_button, button_layout[1]);
            }
        }
    }
}

fn render_path_input_modal(frame: &mut Frame, app: &App, area: Rect) {
    if app.focus == PanelFocus::PathInputModal {
        let popup_block = styled_block("Search Directory Path", true);
        let centered_area = area.centered(Constraint::Percentage(60), Constraint::Max(11));
        frame.render_widget(Clear, centered_area);

        // text area + button bar
        let layout = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(popup_block.inner(centered_area));

        // outer block
        frame.render_widget(
            Block::default()
                .style(Style::default().bg(Color::Rgb(46, 46, 46)))
                .borders(Borders::ALL),
            centered_area,
        );
        frame.render_widget(popup_block, centered_area);

        // current path
        let curr = format!(" {}", &app.data.selected_entry_dir);
        let current_path = Paragraph::new(curr)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        frame.render_widget(current_path, layout[0]);

        // path input
        let search_style = if app.path_input.query.is_empty() {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::White)
        };
        let search_label: String = "Path (absolute path)".to_string();
        let search_display = if app.path_input.query.is_empty() {
            " [Start typing path of the directory where you want to initiate the search]"
                .to_string()
        } else {
            format!(" {}", app.path_input.query)
        };
        let search_bar = Paragraph::new(search_display)
            .style(search_style)
            .block(Block::bordered().title(search_label));
        frame.render_widget(search_bar, layout[1]);

        // read render_search_input version for detail
        let x_pos: u16 = if app.path_input.query.is_empty() {
            layout[1].x + 1
        } else {
            layout[1].x + app.path_input.char_index as u16 + 2
        };
        frame.set_cursor_position(Position::new(
            x_pos,
            // Move one line down, from the border to the input line
            layout[1].y + 1,
        ));

        // buttons
        let button_layout =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(layout[2]);

        let cancel_button = Paragraph::new("Cancel, don't update [Esc]")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .style(Style::default().fg(Color::Red));

        let update_button = Paragraph::new("Save, update path [Enter]")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .style(Style::default().fg(Color::Green));

        frame.render_widget(cancel_button, button_layout[0]);
        frame.render_widget(update_button, button_layout[1]);
    }
}

fn render_artifact_input_modal(frame: &mut Frame, app: &App, area: Rect) {
    if app.focus == PanelFocus::ArtifactInputModal {
        let popup_block = styled_block("Create custom search artifact", true);
        let centered_area = area.centered(Constraint::Percentage(60), Constraint::Max(8));
        frame.render_widget(Clear, centered_area);

        // input + button bar
        let layout = Layout::vertical([Constraint::Length(3), Constraint::Length(3)])
            .split(popup_block.inner(centered_area));

        // outer block
        frame.render_widget(
            Block::default()
                .style(Style::default().bg(Color::Rgb(46, 46, 46)))
                .borders(Borders::ALL),
            centered_area,
        );
        frame.render_widget(popup_block, centered_area);

        // artifact name input
        let input_style = if app.artifact_input.query.is_empty() {
            Style::default().fg(Color::Gray)
        } else {
            Style::default().fg(Color::Yellow)
        };
        let input_label = "Custom directory name".to_string();
        let input_display = if app.artifact_input.query.is_empty() {
            " [Enter custom directory name]".to_string()
        } else {
            format!(" {}", app.artifact_input.query)
        };
        let input_bar = Paragraph::new(input_display)
            .style(input_style)
            .block(Block::bordered().title(input_label));
        frame.render_widget(input_bar, layout[0]);

        // cursor
        let x_pos: u16 = if app.artifact_input.query.is_empty() {
            layout[0].x + 1
        } else {
            layout[0].x + app.artifact_input.char_index as u16 + 2
        };
        frame.set_cursor_position(Position::new(x_pos, layout[0].y + 1));

        // buttons
        let button_layout =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(layout[1]);

        let cancel_button = Paragraph::new("Cancel, don't add [Esc]")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .style(Style::default().fg(Color::Red));

        let update_button = Paragraph::new("Save, add directory [Enter]")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .style(Style::default().fg(Color::Green));

        frame.render_widget(cancel_button, button_layout[0]);
        frame.render_widget(update_button, button_layout[1]);
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
