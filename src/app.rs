use crate::{
    data::Data,
    model::{
        artifact::ArtifactKind,
        language::Language,
        scan::{
            DeleteState, DeletedEntries, ErrorMetadata, ScanEntry, ScanResult, ScanState,
            ScanStatistics, ScanTraversalState,
        },
    },
    ui::{
        draw,
        input::InputState,
        state::{PanelFocus, StatefulList},
    },
    utils::entry_matches_query,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use jwalk::{DirEntry, WalkDir};
use ratatui::{DefaultTerminal, widgets::TableState};
use std::{
    collections::HashSet,
    fs, io,
    path::Path,
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
    scan_recv: Option<Receiver<ScanState>>,
    delete_recv: Option<Receiver<DeleteState>>,
    pub tick: u64,
    pub show_input_modal: bool,
    pub table_state: TableState,
    pub search_input: InputState,
    pub search_focused: bool,
    pub selected_entries: HashSet<String>,
    pub selected_size: u64,
    pub delete_state: DeleteState,
    pub data: Data,
    pub path_input: InputState,
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
            scan_recv: None,
            delete_recv: None,
            tick: 0,
            show_input_modal: false,
            table_state,
            search_input: InputState::default(),
            search_focused: false,
            selected_entries: HashSet::new(),
            selected_size: 0,
            delete_state: DeleteState::None,
            data: Data::default(),
            path_input: InputState::default(),
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            self.handle_threads_events();
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
                } else if self.focus == PanelFocus::PathInputModal {
                    self.handle_path_input_keys(key);
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
                                if self.delete_state == DeleteState::Confirmation {
                                    self.delete_dir();
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Char('n') => match self.scan_state {
                            ScanState::Confirmation => self.scan_state = ScanState::Idle,
                            ScanState::Completed(_) => {
                                if self.delete_state == DeleteState::Confirmation {
                                    self.delete_state = DeleteState::None
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Char('a') => {
                            if let ScanState::Completed(scan_result) = &self.scan_state {
                                scan_result.scanned_entries.iter().for_each(|entry| {
                                    self.selected_entries.insert(entry.path.to_string());
                                });
                                self.selected_size = scan_result.total_size;
                            }
                        }
                        KeyCode::Char('x') => {
                            if let ScanState::Completed(_) = &self.scan_state {
                                self.selected_entries.clear();
                                self.selected_size = 0;
                            }
                        }
                        KeyCode::Char('d') => self.open_deletion_modal(),
                        KeyCode::Char('p') => {
                            if self.scan_state == ScanState::Idle
                                && self.focus == PanelFocus::Results
                            {
                                self.focus = PanelFocus::PathInputModal
                            }
                        }
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

    fn handle_threads_events(&mut self) {
        if let Some(rx) = &self.scan_recv
            && let Ok(scan_state) = rx.try_recv()
        {
            self.scan_state = scan_state;
        };

        if let Some(rx) = &self.delete_recv
            && let Ok(delete_state) = rx.try_recv()
        {
            // get the deleted entries from the channel and sync them with stored entries
            // state, after the deletion has been completed
            if let DeleteState::Completed(ref deleted) = delete_state
                && let ScanState::Completed(ref mut result) = self.scan_state
            {
                result
                    .scanned_entries
                    .retain(|e| !deleted.contains(&e.path));

                result.total_size = result.scanned_entries.iter().map(|x| x.size).sum();

                self.selected_size = result
                    .scanned_entries
                    .iter()
                    .map(|x| {
                        if self.selected_entries.contains(&x.path) {
                            x.size
                        } else {
                            0
                        }
                    })
                    .sum();
            }

            self.delete_state = delete_state;
        };
    }

    fn scan_dir(&mut self) {
        // to not block main thread, we offload directory scanning to a spawned thread
        // and to access the scan state from the main thread, we use a channel
        let (tx, rx) = mpsc::channel::<ScanState>();
        self.scan_recv = Some(rx);

        let dir = self.data.selected_entry_dir.clone();
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
                    match entry {
                        Ok(entry) => {
                            Self::calculate_stats(entry, artifact.clone(), &mut scan_stats)
                        }
                        Err(err) => {
                            let path = err.path().unwrap_or(Path::new("")).display();
                            error!("failed to access entry {}", path);
                            if let Some(inner) = err.io_error() {
                                match inner.kind() {
                                    io::ErrorKind::InvalidData => {
                                        error!("entry contains invalid data: {}", inner)
                                    }
                                    io::ErrorKind::PermissionDenied => {
                                        error!("Missing permission to read entry: {}", inner)
                                    }
                                    io::ErrorKind::NotFound => {
                                        error!("File or directory not found: {}", inner);
                                        // will probably occur when a wrong entry directory path is
                                        // selected, so we let the user know and stop further scanning

                                        let metadata = ErrorMetadata {
                                            message: format!(
                                                "The selected directory was not found. Are you sure it exists?"
                                            ),
                                            path: Some(path.to_string()),
                                        };
                                        let _ = tx.send(ScanState::Error(metadata));

                                        return; // return from this spawned thread
                                    }
                                    _ => {
                                        error!("Unexpected error occurred: {}", inner)
                                    }
                                }
                            }
                        }
                    }
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
            let metadata = ErrorMetadata {
                message: String::from("Error scanning the selected artifact."),
                path: None,
            };
            tx.send(ScanState::Error(metadata)).ok();
        }
    }

    // todo: add tests
    fn calculate_stats(
        entry: DirEntry<((), ())>,
        artifact: ArtifactKind,
        scan_stats: &mut ScanStatistics,
    ) {
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

    fn handle_input_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.search_focused = false;
            }
            KeyCode::Char(to_insert) => self.search_input.enter_char(to_insert),
            KeyCode::Left => self.search_input.move_cursor_left(),
            KeyCode::Right => self.search_input.move_cursor_right(),
            KeyCode::Backspace => self.search_input.delete_search_char(),
            _ => {}
        }
    }

    fn handle_path_input_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.focus = PanelFocus::Results;
                // flush input
                self.path_input.clear();
            }
            KeyCode::Enter => {
                self.focus = PanelFocus::Results;
                // todo: handle error case too when jwalkdir encounters invalid path
                self.data.update_dir(self.path_input.query.clone());
                self.path_input.clear();
            }
            KeyCode::Char(c) => self.path_input.enter_char(c),
            KeyCode::Left => self.path_input.move_cursor_left(),
            KeyCode::Right => self.path_input.move_cursor_right(),
            KeyCode::Backspace => self.path_input.delete_search_char(),
            _ => {}
        }
    }

    fn open_deletion_modal(&mut self) {
        if let ScanState::Completed(_) = &self.scan_state
            && !self.selected_entries.is_empty()
            && self.delete_state == DeleteState::None
        {
            self.delete_state = DeleteState::Confirmation;
            self.focus = PanelFocus::DeleteModal
        }
    }

    fn delete_dir(&mut self) {
        let mut deleted: DeletedEntries = vec![];

        let (tx, rx) = mpsc::channel::<DeleteState>();
        self.delete_recv = Some(rx);
        let mut selected_entries = self.selected_entries.clone();

        let _ = thread::spawn(move || {
            tx.send(DeleteState::InProgress).ok();

            // test
            // let delay = time::Duration::from_secs(2);
            // thread::sleep(delay);

            // delete selected directories
            for entry in selected_entries.drain() {
                if let Err(e) = fs::remove_dir_all(&entry) {
                    error!("Error deleting {}", e);
                } else {
                    debug!("Deleted {}", &entry);
                    deleted.push(entry);
                };
            }

            // transmit deleted entries to receiver
            tx.send(DeleteState::Completed(deleted)).ok();
        });
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
                        let query = self.search_input.query.to_lowercase();
                        let found_entry = result
                            .scanned_entries
                            .iter()
                            .filter(|entry| entry_matches_query(entry, &query))
                            .nth(row);
                        if let Some(curr) = found_entry {
                            if self.selected_entries.contains(&curr.path) {
                                self.selected_entries.remove(&curr.path);
                                self.selected_size -= curr.size;
                            } else {
                                self.selected_entries.insert(curr.path.to_string());
                                self.selected_size += curr.size;
                            }
                        };
                    }
                }
            }
            PanelFocus::DeleteModal => {
                if let DeleteState::Completed(_) = &self.delete_state {
                    self.focus = PanelFocus::Results;
                    self.delete_state = DeleteState::None;
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
            PanelFocus::DeleteModal => PanelFocus::DeleteModal,
            PanelFocus::PathInputModal => PanelFocus::PathInputModal,
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
