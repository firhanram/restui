use crossterm::event::{KeyCode, KeyEvent};

#[derive(Clone, Debug, Default)]
pub struct TextInput {
    value: String,
    cursor: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.len();
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn row_column(&self) -> (usize, usize) {
        let before_cursor = &self.value[..self.cursor];
        let row = before_cursor.bytes().filter(|byte| *byte == b'\n').count();
        let line_start = before_cursor.rfind('\n').map_or(0, |index| index + 1);
        let column = self.value[line_start..self.cursor].chars().count();
        (row, column)
    }

    pub fn handle_key(&mut self, key: KeyEvent, multiline: bool) {
        match key.code {
            KeyCode::Char(character) => self.insert(character),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.left(),
            KeyCode::Right => self.right(),
            KeyCode::Home => self.home(),
            KeyCode::End => self.end(),
            KeyCode::Up if multiline => self.up(),
            KeyCode::Down if multiline => self.down(),
            KeyCode::Enter if multiline => self.insert('\n'),
            KeyCode::Tab if multiline => self.insert_str("  "),
            _ => {}
        }
    }

    pub fn insert(&mut self, character: char) {
        self.value.insert(self.cursor, character);
        self.cursor += character.len_utf8();
    }

    pub fn insert_str(&mut self, text: &str) {
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let previous = self.value[..self.cursor]
            .char_indices()
            .last()
            .map_or(0, |(index, _)| index);
        self.value.drain(previous..self.cursor);
        self.cursor = previous;
    }

    fn delete(&mut self) {
        if self.cursor == self.value.len() {
            return;
        }
        let length = self.value[self.cursor..]
            .chars()
            .next()
            .map_or(0, char::len_utf8);
        self.value.drain(self.cursor..self.cursor + length);
    }

    fn left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.value[..self.cursor]
                .char_indices()
                .last()
                .map_or(0, |(index, _)| index);
        }
    }

    fn right(&mut self) {
        if self.cursor < self.value.len() {
            self.cursor += self.value[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        }
    }

    fn home(&mut self) {
        self.cursor = self.value[..self.cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
    }

    fn end(&mut self) {
        self.cursor = self.value[self.cursor..]
            .find('\n')
            .map_or(self.value.len(), |index| self.cursor + index);
    }

    fn up(&mut self) {
        let (row, column) = self.row_column();
        if row == 0 {
            self.cursor = 0;
            return;
        }
        self.move_to_row_column(row - 1, column);
    }

    fn down(&mut self) {
        let (row, column) = self.row_column();
        if row + 1 >= self.value.lines().count() {
            self.cursor = self.value.len();
            return;
        }
        self.move_to_row_column(row + 1, column);
    }

    fn move_to_row_column(&mut self, row: usize, column: usize) {
        let line_start = if row == 0 {
            0
        } else {
            self.value
                .match_indices('\n')
                .nth(row - 1)
                .map_or(0, |(index, _)| index + 1)
        };
        let line_end = self.value[line_start..]
            .find('\n')
            .map_or(self.value.len(), |index| line_start + index);
        self.cursor = self.value[line_start..line_end]
            .char_indices()
            .nth(column)
            .map_or(line_end, |(index, _)| line_start + index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_at_utf8_character_boundaries() {
        let mut input = TextInput::new("aé🦀");
        input.left();
        input.backspace();
        assert_eq!(input.value(), "a🦀");
        input.insert('文');
        assert_eq!(input.value(), "a文🦀");
    }

    #[test]
    fn moves_between_lines_while_preserving_columns() {
        let mut input = TextInput::new("first\nsecond\nthird");
        input.home();
        input.up();
        assert_eq!(input.row_column(), (1, 0));
        input.up();
        assert_eq!(input.row_column(), (0, 0));
        input.down();
        input.end();
        input.down();
        assert_eq!(input.row_column(), (2, 5));
    }
}
