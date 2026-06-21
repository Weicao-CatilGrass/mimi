pub type LexerError = String;

pub fn tabs_not_allowed(line: usize, col: usize) -> LexerError {
    format!("tabs are not allowed for indentation at {}:{}", line, col)
}

pub fn indent_not_multiple_of_four(line: usize, col: usize) -> LexerError {
    format!("indentation must be a multiple of 4 spaces at {}:{}", line, col)
}

pub fn dedent_mismatch(line: usize, col: usize) -> LexerError {
    format!("dedent does not match any indentation level at {}:{}", line, col)
}

pub fn unexpected_dollar(line: usize, col: usize) -> LexerError {
    format!("unexpected '$' at {}:{}", line, col)
}

pub fn unexpected_character(c: char, line: usize, col: usize) -> LexerError {
    format!("unexpected character '{}' at {}:{}", c, line, col)
}

pub fn unterminated_string() -> LexerError {
    "unterminated string".into()
}

pub fn unterminated_escape() -> LexerError {
    "unterminated escape".into()
}

pub fn unterminated_fstring() -> LexerError {
    "unterminated f-string".into()
}

pub fn unterminated_fstring_escape() -> LexerError {
    "unterminated escape in f-string".into()
}

pub fn unterminated_interpolation() -> LexerError {
    "unterminated interpolation in f-string".into()
}
