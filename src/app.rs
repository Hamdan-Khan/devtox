use crate::{
    model::{ArtifactKind, Language},
    ui::draw,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use jwalk::{DirEntry, Error, WalkDir};
use ratatui::DefaultTerminal;
use std::{
    io,
    sync::mpsc::{self, Receiver},
    thread,
    time::{self, Duration},
};
use tracing::{error, info};

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
    // todo: add focus for scan panel
}

#[derive(Default)]
pub struct ScanResult {
    pub total_size: u64,
    pub symlink_count: u64,
    pub error_count: u64,
}

pub enum ScanState {
    Idle,
    Confirmation,
    InProgress,
    Error,
    Completed(ScanResult),
}

pub struct App {
    pub language_list: StatefulList<Language>,
    pub artifact_list: StatefulList<ArtifactKind>,
    pub focus: PanelFocus,
    pub exit: bool,
    pub scan_result: ScanResult,
    pub scan_state: ScanState,
    pub selected_entry_dir: String,
    scan_recv: Option<Receiver<ScanState>>,
}

impl App {
    pub fn new() -> App {
        let languages = Language::all();
        let initial_artifacts = languages[0].artifacts();

        App {
            language_list: StatefulList::new(languages),
            artifact_list: StatefulList::new(initial_artifacts),
            focus: PanelFocus::Languages,
            exit: false,
            scan_result: ScanResult::default(),
            scan_state: ScanState::Idle,
            selected_entry_dir: String::from("/home/hamdan/Documents/Development/rust/devtox"),
            scan_recv: None,
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
                    KeyCode::Char('s') => match self.scan_state {
                        ScanState::Idle => self.scan_state = ScanState::Confirmation,
                        _ => {}
                    },
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
                    KeyCode::Enter | KeyCode::Right => {
                        if self.focus == PanelFocus::Languages {
                            self.focus = PanelFocus::Artifacts;
                        }
                    }
                    KeyCode::Left | KeyCode::Esc => {
                        if self.focus == PanelFocus::Artifacts {
                            self.focus = PanelFocus::Languages;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_scan_events(&mut self) {
        if let Some(rx) = &self.scan_recv {
            if let Ok(scan_state) = rx.try_recv() {
                if let ScanState::Completed(s) = scan_state {
                    self.scan_result = s;
                    self.scan_state = ScanState::Completed(ScanResult::default());
                } else {
                    self.scan_state = scan_state;
                }
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
                let mut total_size: u64 = 0;
                let mut symlink_count: u64 = 0;
                let mut error_count: u64 = 0;
                let mut is_target_dir: bool = false;
                let mut depth: usize = 0;

                for entry in WalkDir::new(dir) {
                    Self::calculate_stats(
                        entry,
                        artifact.clone(),
                        &mut total_size,
                        &mut symlink_count,
                        &mut error_count,
                        &mut is_target_dir,
                        &mut depth,
                    );
                }

                // for testing
                // let delay = time::Duration::from_secs(1);
                // thread::sleep(delay);

                tx.send(ScanState::Completed(ScanResult {
                    total_size,
                    symlink_count,
                    error_count,
                }))
                .ok();
            });
        } else {
            tx.send(ScanState::Error).ok();
        }
    }

    fn calculate_stats(
        entry: Result<DirEntry<((), ())>, Error>,
        artifact: ArtifactKind,
        total_size: &mut u64,
        symlink_count: &mut u64,
        error_count: &mut u64,
        is_target_dir: &mut bool,
        depth: &mut usize,
    ) {
        match entry {
            Ok(entry) => {
                if let Ok(metadata) = entry.metadata() {
                    let is_target =
                        artifact.display_name() == entry.file_name() && metadata.is_dir();
                    // update global flags
                    if is_target {
                        // todo: handle cases like target dir inside target dir, which one's depth should be kept in the global var?
                        *is_target_dir = true;
                        *depth = entry.depth;
                    }
                    // disable flags when encounter siblings of the target directory i.e. same depth but not the target directory
                    // this won't disable global flags for the target directory itself
                    if !is_target && *depth == entry.depth {
                        *is_target_dir = false;
                    }
                    // disable flag when encounter a parent of the target directory
                    if *is_target_dir && *depth > entry.depth {
                        *is_target_dir = false;
                    }
                    // child, if its depth is strictly greater than the target directory
                    // and walker encounters it after the target directory
                    let is_child = *is_target_dir && entry.depth > *depth;

                    if is_child {
                        info!("{:?}, {}", entry.file_name(), entry.depth);
                        *total_size += metadata.len();
                        if metadata.is_symlink() {
                            *symlink_count += 1;
                        };
                    }
                }
            }
            Err(err) => {
                *error_count += 1;
                error!("Error processing file {:?}", err)
            }
        }
    }

    // to move focus across different panels when tab key is pressed
    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::Languages => PanelFocus::Artifacts,
            PanelFocus::Artifacts => PanelFocus::Languages,
        };
    }

    fn on_down(&mut self) {
        match self.focus {
            PanelFocus::Languages => {
                self.language_list.next();
                self.refresh_artifacts();
            }
            PanelFocus::Artifacts => self.artifact_list.next(),
        }
    }

    fn on_up(&mut self) {
        match self.focus {
            PanelFocus::Languages => {
                self.language_list.previous();
                self.refresh_artifacts();
            }
            PanelFocus::Artifacts => self.artifact_list.previous(),
        }
    }

    // to repopulate artifact panel when selected language changes
    fn refresh_artifacts(&mut self) {
        if let Some(lang) = self.language_list.selected_item() {
            self.artifact_list = StatefulList::new(lang.artifacts());
        }
    }
}
