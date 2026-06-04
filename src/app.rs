use crate::{
    model::{
        artifact::ArtifactKind,
        language::Language,
        scan::{ScanEntry, ScanResult, ScanState, ScanStatistics, ScanTraversalState},
    },
    ui::{
        draw,
        state::{PanelFocus, StatefulList},
    },
    utils::entry_matches_query,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use jwalk::{DirEntry, Error, WalkDir};
use ratatui::{DefaultTerminal, widgets::TableState};
use std::{
    collections::HashSet,
    fs, io,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
    vec,
};
use tracing::{debug, error, info};

pub struct App {
    pub language_list: StatefulList<Language>,
    pub artifact_list: StatefulList<ArtifactKind>,
    pub focus: PanelFocus,
    pub exit: bool,
    pub scan_state: ScanState,
    pub selected_entry_dir: String,
    scan_recv: Option<Receiver<ScanState>>,
    pub tick: u64,
    pub show_input_modal: bool,
    pub table_state: TableState,
    pub search_query: String,
    pub search_focused: bool,
    pub search_char_index: usize,
    pub selected_entries: HashSet<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> App {
        let languages = Language::all();
        let initial_artifacts = languages[0].artifacts();
        let mut table_state = TableState::default();
        table_state.select_first();
        table_state.select_first_column();

        App {
            language_list: StatefulList::new(languages),
            artifact_list: StatefulList::new(initial_artifacts),
            focus: PanelFocus::Languages,
            exit: false,
            scan_state: ScanState::Idle,
            selected_entry_dir: String::from("/home/hamdan/Documents/Development/rust/devtox/test"),
            scan_recv: None,
            tick: 0,
            show_input_modal: false,
            table_state,
            search_query: String::new(),
            search_focused: false,
            search_char_index: 0,
            selected_entries: HashSet::new(),
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            self.handle_scan_events();
            terminal.draw(|frame| draw(frame, self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                // only handle key press, not release
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }
                if self.search_focused {
                    self.handle_input_keys(key);
                } else {
                    match key.code {
                        KeyCode::Char('q') => self.exit = true,
                        KeyCode::Char('s') => {
                            if self.focus == PanelFocus::Results {
                                match self.scan_state {
                                    ScanState::Idle => self.scan_state = ScanState::Confirmation,
                                    ScanState::Completed(_) => {
                                        self.search_focused = true;
                                    }
                                    _ => {}
                                }
                                self.focus = PanelFocus::Results
                            }
                        }
                        KeyCode::Char('y') => match &self.scan_state {
                            ScanState::Confirmation => self.scan_dir(),
                            ScanState::Completed(_) => {
                                if self.focus == PanelFocus::DeletionModal {
                                    self.delete_dir()
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Char('n') => match self.scan_state {
                            ScanState::Confirmation => self.scan_state = ScanState::Idle,
                            ScanState::Completed(_) => {
                                if self.focus == PanelFocus::DeletionModal {
                                    self.focus = PanelFocus::Results
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Char('a') => {
                            if let ScanState::Completed(scan_result) = &self.scan_state {
                                scan_result.scanned_entries.iter().for_each(|entry| {
                                    self.selected_entries.insert(entry.path.to_string());
                                });
                            }
                        }
                        KeyCode::Char('x') => {
                            if let ScanState::Completed(_) = &self.scan_state {
                                self.selected_entries.clear();
                            }
                        }
                        KeyCode::Char('d') => self.open_deletion_modal(),
                        KeyCode::Tab => self.cycle_focus(),
                        KeyCode::Down => self.on_down(),
                        KeyCode::Up => self.on_up(),
                        KeyCode::Right => self.on_right(),
                        KeyCode::Left => self.on_left(),
                        KeyCode::Enter => self.handle_enter_key(),
                        KeyCode::Esc => self.handle_esc_key(),
                        _ => {}
                    }
                }
            }
        } else {
            self.tick = self.tick.wrapping_add(1);
        }
        Ok(())
    }

    fn handle_scan_events(&mut self) {
        if let Some(rx) = &self.scan_recv
            && let Ok(scan_state) = rx.try_recv()
        {
            self.scan_state = scan_state;
        };
    }

    fn scan_dir(&mut self) {
        // to not block main thread, we offload directory scanning to a spawned thread
        // and to access the scan state from the main thread, we use a channel
        let (tx, rx) = mpsc::channel::<ScanState>();
        self.scan_recv = Some(rx);

        let dir = self.selected_entry_dir.clone();
        let artifact = self.artifact_list.selected_item().cloned();

        if let Some(artifact) = artifact {
            thread::spawn(move || {
                tx.send(ScanState::InProgress).ok();
                let mut scan_stats = ScanStatistics {
                    total_size: 0,
                    symlink_count: 0,
                    error_count: 0,
                    is_target_dir: false,
                    depth: 0,
                    scanned_entries: vec![],
                    traversal_state: ScanTraversalState::Outside,
                };

                for entry in WalkDir::new(dir) {
                    Self::calculate_stats(entry, artifact.clone(), &mut scan_stats);
                }

                // if the last element scanned is target directory or its child, the traversal state is outdated
                // and accumulation of entry is not performed
                if let ScanTraversalState::Inside {
                    accumulated_size,
                    path,
                } = scan_stats.traversal_state
                {
                    scan_stats.scanned_entries.push(ScanEntry {
                        path: path.to_string(),
                        size: accumulated_size,
                    });
                }

                debug!(
                    "{}\n{:?}",
                    &scan_stats.symlink_count, &scan_stats.scanned_entries,
                );

                // for testing
                // let delay = time::Duration::from_secs(2);
                // thread::sleep(delay);

                tx.send(ScanState::Completed(ScanResult {
                    total_size: scan_stats.total_size,
                    symlink_count: scan_stats.symlink_count,
                    error_count: scan_stats.error_count,
                    scanned_entries: scan_stats.scanned_entries,
                }))
                .ok();
            });
        } else {
            tx.send(ScanState::Error).ok();
        }
    }

    // todo: add tests
    fn calculate_stats(
        entry: Result<DirEntry<((), ())>, Error>,
        artifact: ArtifactKind,
        scan_stats: &mut ScanStatistics,
    ) {
        match entry {
            Ok(entry) => {
                if let Ok(metadata) = entry.metadata() {
                    let entry_depth = entry.depth;
                    let entry_size = metadata.len();
                    let is_symlink = metadata.is_symlink();
                    // symbolic links are non-dirs, but we'll count them as a target
                    let is_target = if is_symlink {
                        artifact.display_name() == entry.file_name()
                    } else {
                        artifact.display_name() == entry.file_name() && metadata.is_dir()
                    };
                    // update global flags
                    if is_target {
                        // if we're walking inside the target directory and encounter another instance of target directory as its child e.g. target/xyz/target,
                        // no need to update the global depth because the global depth should track the parent target directory's depth
                        if !scan_stats.is_target_dir {
                            scan_stats.depth = entry_depth;
                        }
                        scan_stats.is_target_dir = true;
                    } else {
                        // disable flags when encounter siblings of the target directory i.e. same depth but not the target directory
                        // this won't disable global flags for the target directory itself
                        if scan_stats.depth == entry_depth {
                            scan_stats.is_target_dir = false;
                        }
                        // disable flag when encounter a parent of the target directory
                        if scan_stats.is_target_dir && scan_stats.depth > entry_depth {
                            scan_stats.is_target_dir = false;
                        }
                    }

                    // pattern matching state machine for accumulating entry-wise data i.e. size and path for scanned target entries
                    match (&scan_stats.traversal_state, scan_stats.is_target_dir) {
                        // walker encounters the target directory
                        (ScanTraversalState::Outside, true) => {
                            scan_stats.traversal_state = ScanTraversalState::Inside {
                                accumulated_size: entry_size,
                                path: entry.path().display().to_string(),
                            }
                        }
                        // walker is inside the target directory
                        (
                            ScanTraversalState::Inside {
                                accumulated_size,
                                path,
                            },
                            true,
                        ) => {
                            scan_stats.traversal_state = ScanTraversalState::Inside {
                                accumulated_size: accumulated_size + entry_size,
                                path: path.to_string(),
                            }
                        }
                        // walker leaves the target directory
                        (
                            ScanTraversalState::Inside {
                                accumulated_size,
                                path,
                            },
                            false,
                        ) => {
                            scan_stats.scanned_entries.push(ScanEntry {
                                path: path.to_string(),
                                size: *accumulated_size,
                            });
                            scan_stats.traversal_state = ScanTraversalState::Outside;
                        }
                        _ => {}
                    }

                    if scan_stats.is_target_dir && is_symlink {
                        scan_stats.symlink_count += 1;
                    };

                    // child, if its depth is strictly greater than the target directory
                    // and walker encounters it after the target directory
                    let is_child = scan_stats.is_target_dir && entry_depth > scan_stats.depth;

                    if is_child {
                        info!("{:?}, {}", entry.file_name(), entry_depth);
                        scan_stats.total_size += entry_size;
                    }
                }
            }
            Err(err) => {
                scan_stats.error_count += 1;
                error!("Error processing file {:?}", err)
            }
        }
    }

    fn move_cursor_left(&mut self) {
        if self.search_char_index > 0 {
            self.search_char_index -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.search_char_index < self.search_query.len() {
            self.search_char_index += 1;
        }
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.search_query
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.search_char_index)
            .unwrap_or(self.search_query.len())
    }

    fn enter_char(&mut self, to_insert: char) {
        let index = self.byte_index();
        self.search_query.insert(index, to_insert);
        self.move_cursor_right();
    }

    fn delete_search_char(&mut self) {
        let is_not_cursor_leftmost = self.search_char_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.search_char_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.search_query.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.search_query.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.search_query = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn handle_input_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.search_focused = false;
            }
            KeyCode::Char(to_insert) => self.enter_char(to_insert),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Backspace => self.delete_search_char(),
            _ => {}
        }
    }

    fn open_deletion_modal(&mut self) {
        if let ScanState::Completed(_) = &self.scan_state
            && !self.selected_entries.is_empty()
        {
            self.focus = PanelFocus::DeletionModal
        }
    }

    fn delete_dir(&mut self) {
        // close the confirmation modal
        self.focus = PanelFocus::Results;
        let mut deleted: Vec<String> = vec![];

        for entry in self.selected_entries.drain() {
            if let Err(e) = fs::remove_dir_all(&entry) {
                error!("Error deleting {}", e);
            } else {
                debug!("Deleted {}", &entry);
                deleted.push(entry);
            };
        }

        if let ScanState::Completed(ref mut result) = self.scan_state {
            result
                .scanned_entries
                .retain(|e| !deleted.contains(&e.path));
        }
    }

    fn handle_enter_key(&mut self) {
        match self.focus {
            PanelFocus::Languages => {
                self.focus = PanelFocus::Artifacts;
            }
            PanelFocus::Artifacts => {
                // if "New" artifact is selected, show the input modal. otherwise, focus the scan panel
                if let Some(selected_artifact) = self.artifact_list.selected_item() {
                    // to create custom artifact
                    if *selected_artifact == ArtifactKind::New {
                        self.show_input_modal = true;
                        self.focus = PanelFocus::InputModal
                    } else {
                        self.focus = PanelFocus::Results;
                    }
                }
            }
            PanelFocus::Results => {
                // to toggle entries in the scanned table
                if let ScanState::Completed(result) = &self.scan_state {
                    if let Some(row) = self.table_state.selected() {
                        let query = self.search_query.to_lowercase();
                        let found_entry = result
                            .scanned_entries
                            .iter()
                            .filter(|entry| entry_matches_query(entry, &query))
                            .nth(row);
                        if let Some(curr) = found_entry {
                            if self.selected_entries.contains(&curr.path) {
                                self.selected_entries.remove(&curr.path);
                            } else {
                                self.selected_entries.insert(curr.path.to_string());
                            }
                        };
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_esc_key(&mut self) {
        // esc shouldn't work when scanning (or deletion when we later add it) is in progress
        if self.focus == PanelFocus::Results && self.scan_state != ScanState::InProgress {
            self.focus = PanelFocus::Languages;
            self.scan_state = ScanState::Idle;
        } else if self.focus == PanelFocus::InputModal {
            self.show_input_modal = false;
            self.focus = PanelFocus::Artifacts
        }
    }

    // to move focus across different panels when tab key is pressed
    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Languages => PanelFocus::Artifacts,
            PanelFocus::Artifacts => PanelFocus::Languages,
            PanelFocus::Results => PanelFocus::Results,
            PanelFocus::InputModal => PanelFocus::InputModal,
            PanelFocus::DeletionModal => PanelFocus::DeletionModal,
        };
    }

    fn on_down(&mut self) {
        match (&self.focus, &self.scan_state) {
            (PanelFocus::Results, ScanState::Completed(_)) => {
                self.table_state.select_next();
            }
            (PanelFocus::Languages, _) => {
                self.language_list.next();
                self.refresh_artifacts();
            }
            (PanelFocus::Artifacts, _) => self.artifact_list.next(),
            _ => {}
        }
    }

    fn on_up(&mut self) {
        match (&self.focus, &self.scan_state) {
            (PanelFocus::Results, ScanState::Completed(_)) => {
                self.table_state.select_previous();
            }
            (PanelFocus::Languages, _) => {
                self.language_list.previous();
                self.refresh_artifacts();
            }
            (PanelFocus::Artifacts, _) => self.artifact_list.previous(),
            _ => {}
        }
    }

    fn on_left(&mut self) {
        if let (PanelFocus::Results, ScanState::Completed(_)) = (&self.focus, &self.scan_state) {
            self.table_state.select_previous_column();
        }
    }

    fn on_right(&mut self) {
        if let (PanelFocus::Results, ScanState::Completed(_)) = (&self.focus, &self.scan_state) {
            self.table_state.select_next_column();
        }
    }

    // to repopulate artifact panel when selected language changes
    fn refresh_artifacts(&mut self) {
        if let Some(lang) = self.language_list.selected_item() {
            self.artifact_list = StatefulList::new(lang.artifacts());
        }
    }
}
