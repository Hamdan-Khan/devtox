use crate::{
    model::{ArtifactKind, Language},
    ui::draw,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use jwalk::{DirEntry, Error, WalkDir};
use ratatui::{DefaultTerminal, widgets::TableState};
use std::{
    io,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};
use tracing::{debug, error, info};

// stateful wrapper around vector to keep track of the selected item
// (will use in languages and artifacts sections for now)
#[derive(Clone)]
pub struct StatefulList<T> {
    pub items: Vec<T>,
    pub selected: usize,
}

impl<T> StatefulList<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items, selected: 0 }
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() {
            // cyrcles back to 0th index when overflows
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.items.is_empty() {
            // cycles back to n-1th index when underflows
            if self.selected == 0 {
                self.selected = self.items.len() - 1
            } else {
                self.selected -= 1;
            };
        }
    }

    pub fn selected_item(&self) -> Option<&T> {
        self.items.get(self.selected)
    }
}

// the input panels on the left side
#[derive(PartialEq)]
pub enum PanelFocus {
    Languages,
    Artifacts,
    Results,
    InputModal,
}

#[derive(PartialEq, Debug)]
pub struct ScanEntry {
    pub path: String,
    pub size: u64,
}

enum ScanTraversalState {
    Outside,
    Inside { accumulated_size: u64, path: String },
}

#[derive(Default, PartialEq)]
pub struct ScanResult {
    pub total_size: u64,
    pub symlink_count: u64,
    pub error_count: u64,
    pub scanned_entries: Vec<ScanEntry>,
}

#[derive(PartialEq)]
pub enum ScanState {
    Idle,
    Confirmation,
    InProgress,
    Error,
    Completed(ScanResult),
}

struct ScanStatistics {
    total_size: u64,
    symlink_count: u64,
    error_count: u64,
    is_target_dir: bool,
    depth: usize,
    scanned_entries: Vec<ScanEntry>,
    traversal_state: ScanTraversalState,
}

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
            selected_entry_dir: String::from("/home/hamdan/Documents/Development/rust/devtox/as"),
            scan_recv: None,
            tick: 0,
            show_input_modal: false,
            table_state,
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
                match key.code {
                    KeyCode::Char('q') => self.exit = true,
                    KeyCode::Char('s') => {
                        if self.focus == PanelFocus::Results {
                            match self.scan_state {
                                ScanState::Idle => self.scan_state = ScanState::Confirmation,
                                _ => {}
                            }
                            self.focus = PanelFocus::Results
                        }
                    }
                    KeyCode::Char('y') => match self.scan_state {
                        ScanState::Confirmation => self.scan_dir(),
                        _ => {}
                    },
                    KeyCode::Char('n') => match self.scan_state {
                        ScanState::Confirmation => self.scan_state = ScanState::Idle,
                        _ => {}
                    },
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
        } else {
            self.tick = self.tick.wrapping_add(1);
        }
        Ok(())
    }

    fn handle_scan_events(&mut self) {
        if let Some(rx) = &self.scan_recv {
            if let Ok(scan_state) = rx.try_recv() {
                self.scan_state = scan_state;
            };
        }
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

                debug!("{:?}", &scan_stats.scanned_entries);

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
                    let is_target =
                        artifact.display_name() == entry.file_name() && metadata.is_dir();
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

                    // child, if its depth is strictly greater than the target directory
                    // and walker encounters it after the target directory
                    let is_child = scan_stats.is_target_dir && entry_depth > scan_stats.depth;

                    // todo: might infer total size from sum of all scanned entries
                    if is_child {
                        info!("{:?}, {}", entry.file_name(), entry_depth);
                        scan_stats.total_size += entry_size;
                        if metadata.is_symlink() {
                            scan_stats.symlink_count += 1;
                        };
                    }
                }
            }
            Err(err) => {
                scan_stats.error_count += 1;
                error!("Error processing file {:?}", err)
            }
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
        match (&self.focus, &self.scan_state) {
            (PanelFocus::Results, ScanState::Completed(_)) => {
                self.table_state.select_previous_column();
            }
            _ => {}
        }
    }

    fn on_right(&mut self) {
        match (&self.focus, &self.scan_state) {
            (PanelFocus::Results, ScanState::Completed(_)) => {
                self.table_state.select_next_column();
            }
            _ => {}
        }
    }

    // to repopulate artifact panel when selected language changes
    fn refresh_artifacts(&mut self) {
        if let Some(lang) = self.language_list.selected_item() {
            self.artifact_list = StatefulList::new(lang.artifacts());
        }
    }
}
