pub struct InputState {
    pub query: String,
    pub char_index: usize,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            query: String::new(),
            char_index: 0,
        }
    }
}

impl InputState {
    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.query
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.char_index)
            .unwrap_or(self.query.len())
    }

    pub fn move_cursor_left(&mut self) {
        if self.char_index > 0 {
            self.char_index -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.char_index < self.query.len() {
            self.char_index += 1;
        }
    }

    pub fn enter_char(&mut self, c: char) {
        let index = self.byte_index();
        self.query.insert(index, c);
        self.move_cursor_right();
    }

    pub fn delete_search_char(&mut self) {
        let is_not_cursor_leftmost = self.char_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.char_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.query.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.query.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.query = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.char_index = 0;
    }
}
