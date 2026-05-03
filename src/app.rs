use crate::{
    model::{ArtifactKind, Language},
    ui::draw,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use jwalk::WalkDir;
use ratatui::DefaultTerminal;
use std::{io, thread, time};

// stateful wrapper around vector to keep track of the selected item
// (will use in languages and artifacts sections for now)
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
    Completed,
}

pub struct App {
    pub language_list: StatefulList<Language>,
    pub artifact_list: StatefulList<ArtifactKind>,
    pub focus: PanelFocus,
    pub exit: bool,
    pub scan_result: ScanResult,
    pub scan_state: ScanState,
    pub selected_entry_dir: String,
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
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| draw(frame, self))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
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
        Ok(())
    }

    fn scan_dir(&mut self) {
        self.scan_state = ScanState::InProgress;
        let mut total_size: u64 = 0;
        let mut symlink_count: u64 = 0;
        let mut error_count: u64 = 0;

        for entry in WalkDir::new(&self.selected_entry_dir) {
            match entry {
                Ok(entry) => {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_symlink() {
                            symlink_count += 1;
                        };
                        let size = metadata.len();
                        total_size += size;
                    }
                }
                Err(err) => {
                    error_count += 1;
                    eprintln!("Error processing file {:?}", err)
                }
            }
        }

        let delay = time::Duration::from_secs(1);
        thread::sleep(delay);

        self.scan_state = ScanState::Completed;

        self.scan_result = ScanResult {
            total_size,
            symlink_count,
            error_count,
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
