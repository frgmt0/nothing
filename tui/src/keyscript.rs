
use std::fmt;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::AppState;
use crate::keys::handle_key;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeyScriptError {
    pub line: usize,
    pub text: String,
    pub message: String,
}

impl fmt::Display for KeyScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}: {}", self.line, self.message, self.text)
    }
}

impl std::error::Error for KeyScriptError {}

pub fn parse_key(token: &str) -> Option<KeyEvent> {
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Some(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
    }
    let lower = token.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("c-") {
        let mut chars = rest.chars();
        return match (chars.next(), chars.next()) {
            (Some(c), None) => Some(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)),
            _ => None,
        };
    }
    let code = match lower.as_str() {
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "s-tab" | "shift-tab" | "backtab" => {
            return Some(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        }
        "enter" | "return" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "bksp" | "backspace" => KeyCode::Backspace,
        "del" | "delete" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        _ => return None,
    };
    Some(KeyEvent::new(code, KeyModifiers::NONE))
}

pub fn key_name(key: &KeyEvent) -> String {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char(' ') if !ctrl => "space".to_string(),
        KeyCode::Char(c) if ctrl => format!("C-{c}"),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "s-tab".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Backspace => "bksp".to_string(),
        KeyCode::Delete => "del".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        other => format!("{other:?}").to_ascii_lowercase(),
    }
}

pub fn parse_keys(text: &str) -> Result<Vec<KeyEvent>, KeyScriptError> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let token = trimmed
            .split_whitespace()
            .next()
            .expect("a non-blank line has a first word");
        match parse_key(token) {
            Some(key) => out.push(key),
            None => {
                return Err(KeyScriptError {
                    line: i + 1,
                    text: line.to_string(),
                    message: format!("unknown keystroke `{token}`"),
                });
            }
        }
    }
    Ok(out)
}

pub fn replay_keys(text: &str, state: AppState) -> Result<AppState, KeyScriptError> {
    Ok(parse_keys(text)?
        .into_iter()
        .fold(state, |state, key| handle_key(key, state)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nothing_core::render::render;

    #[test]
    fn a_single_character_is_itself() {
        assert_eq!(
            parse_key("x"),
            Some(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE))
        );
        assert_eq!(parse_key("\\").map(|k| k.code), Some(KeyCode::Char('\\')));
        assert_eq!(parse_key("#").map(|k| k.code), Some(KeyCode::Char('#')));
    }

    #[test]
    fn named_and_control_keys_round_trip() {
        for token in [
            "x", "0", "\\", ";", "space", "tab", "s-tab", "enter", "esc", "bksp", "del", "up",
            "down", "left", "right", "C-z", "C-r", "C-q",
        ] {
            let key = parse_key(token).unwrap_or_else(|| panic!("`{token}` did not parse"));
            assert_eq!(key_name(&key), token, "`{token}` did not round trip");
        }


        assert_eq!(parse_key("TAB"), parse_key("tab"));
        assert_eq!(parse_key("backspace"), parse_key("bksp"));
        assert_eq!(parse_key("c-z"), parse_key("C-z"));
    }

    #[test]
    fn comments_and_blank_lines_do_not_count_as_keystrokes() {
        let script = "# a comment\n\n1   # type a one\n+\n2\n";
        let keys = parse_keys(script).unwrap();
        assert_eq!(keys.len(), 3, "three keystrokes, three lines that count");
        let state = replay_keys(script, AppState::empty()).unwrap();
        assert_eq!(render(&state.program()), "1 + 2");
        assert_eq!(state.keystrokes(), 3);
    }

    #[test]
    fn an_unknown_token_reports_its_line() {
        let err = parse_keys("1\nfrobnicate\n").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("frobnicate"));
    }
}