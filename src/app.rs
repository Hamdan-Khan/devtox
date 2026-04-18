use crate::{
    model::{ArtifactKind, Language},
    ui::draw,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use std::io;

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

pub struct App {
    pub language_list: StatefulList<Language>,
    pub artifact_list: StatefulList<ArtifactKind>,
    pub focus: PanelFocus,
    pub exit: bool,
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
