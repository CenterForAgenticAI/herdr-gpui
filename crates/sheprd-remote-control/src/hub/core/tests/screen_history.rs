//! A Herdr 2 screen keeps history: a phone reads it back or scrolls a view
//! through it, and the shared emulator stays live for everyone else.

use super::*;
use sheprd_terminal::{Emulator, Size};

fn numbered(lines: usize) -> Emulator {
    let mut emulator = Emulator::new(Size::new(10, 3).unwrap(), 100);
    let text = (1..=lines)
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\r\n");
    emulator.feed(text.as_bytes());
    emulator
}

fn plain(read: &Value) -> Vec<String> {
    let text = read["read"]["text"].as_str().unwrap();
    crate::ansi::parse(text)
        .lines
        .iter()
        .map(|runs| {
            let line: String = runs.iter().map(|(_, text)| text.as_str()).collect();
            line.trim_end().to_owned()
        })
        .collect()
}

#[test]
fn recent_history_reads_back_oldest_first_down_to_the_screen() {
    let mut emulator = numbered(10);
    let read = screen::read(&mut emulator, &json!({"source": "recent", "lines": 1000}));
    assert_eq!(read["read"]["history"], 7);
    let expected = (1..=10).map(|line| line.to_string()).collect::<Vec<_>>();
    assert_eq!(plain(&read), expected);
    let read = screen::read(&mut emulator, &json!({"source": "recent", "lines": 4}));
    assert_eq!(plain(&read), ["4", "5", "6", "7", "8", "9", "10"]);
}

#[test]
fn a_scrolled_read_shows_history_and_leaves_the_screen_live() {
    let mut emulator = numbered(10);
    let read = screen::read(&mut emulator, &json!({"scroll": 2}));
    assert_eq!(plain(&read), ["6", "7", "8"]);
    // Past the top shows the top.
    let read = screen::read(&mut emulator, &json!({"scroll": 500}));
    assert_eq!(plain(&read), ["1", "2", "3"]);
    let read = screen::read(&mut emulator, &json!({}));
    assert_eq!(plain(&read), ["8", "9", "10"]);
}

#[test]
fn a_screen_without_history_reads_only_itself() {
    let mut emulator = numbered(2);
    let read = screen::read(&mut emulator, &json!({"source": "recent", "lines": 1000}));
    assert_eq!(read["read"]["history"], 0);
    assert_eq!(plain(&read), ["1", "2", ""]);
}
