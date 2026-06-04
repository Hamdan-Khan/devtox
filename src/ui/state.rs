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
