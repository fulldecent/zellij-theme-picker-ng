//! Shortcut bar painted with the same colors as the status bar.
//!
//! Zellij has no bar widget. The keys are reverse-video tokens. A label shares
//! that key's click, and hovering either one paints both in the key colors.

use unicode_width::UnicodeWidthStr;
use zellij_tile::prelude::{PaletteColor, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Swap,
    Up,
    Down,
    Cancel,
    Apply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    pub bar: Ink,
    pub dark: Ink,
    pub light: Ink,
    pub ctrl: Ink,
    pub alt: Ink,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            bar: Ink::Bit(16),
            dark: Ink::Bit(16),
            light: Ink::Bit(245),
            ctrl: Ink::Bit(255),
            alt: Ink::Bit(166),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Bit(u8),
    Rgb(u8, u8, u8),
}

impl Ink {
    fn fg(self) -> String {
        match self {
            Ink::Bit(n) => format!("38;5;{n}"),
            Ink::Rgb(r, g, b) => format!("38;2;{r};{g};{b}"),
        }
    }

    fn bg(self) -> String {
        match self {
            Ink::Bit(n) => format!("48;5;{n}"),
            Ink::Rgb(r, g, b) => format!("48;2;{r};{g};{b}"),
        }
    }
}

pub fn chrome_from(style: &Style) -> Chrome {
    let colors = &style.colors;
    Chrome {
        bar: ink(colors.text_unselected.background),
        dark: ink(colors.ribbon_unselected.base),
        light: ink(colors.ribbon_unselected.background),
        ctrl: ink(colors.text_unselected.base),
        alt: ink(colors.text_unselected.emphasis_0),
    }
}

fn ink(color: PaletteColor) -> Ink {
    match color {
        PaletteColor::EightBit(n) => Ink::Bit(n),
        PaletteColor::Rgb((r, g, b)) => Ink::Rgb(r, g, b),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Plain,
    Reverse,
}

struct Chunk {
    text: String,
    look: Look,
    hit: Option<Click>,
}

impl Chunk {
    fn new(text: &str, look: Look, hit: Option<Click>) -> Self {
        Self {
            text: text.to_string(),
            look,
            hit,
        }
    }
}

/// The wide line, or keys only when that line does not fit.
pub fn render(
    cols: usize,
    chrome: &Chrome,
    hover: Option<usize>,
) -> (String, Vec<(usize, usize, Click)>) {
    if cols == 0 {
        return (String::new(), Vec::new());
    }
    let mut chunks = if width(&wide()) <= cols {
        wide()
    } else {
        narrow()
    };
    while width(&chunks) > cols && chunks.len() > 1 {
        chunks.pop();
    }
    if chunks.len() == 1 && width(&chunks) > cols {
        chunks[0].text = truncate(&chunks[0].text, cols);
    }
    let gap = cols.saturating_sub(width(&chunks));
    if gap > 0 {
        chunks.push(Chunk::new(&" ".repeat(gap), Look::Plain, None));
    }
    let hits = hits_of(&chunks);
    (paint(&chunks, chrome, hover), hits)
}

pub fn hit(cols: usize, chrome: &Chrome, column: usize) -> Option<Click> {
    let (_, hits) = render(cols, chrome, None);
    hits.iter()
        .find(|(start, end, _)| column >= *start && column < *end)
        .map(|(_, _, click)| *click)
}

fn wide() -> Vec<Chunk> {
    vec![
        Chunk::new("Tab", Look::Reverse, Some(Click::Swap)),
        Chunk::new("↑", Look::Reverse, Some(Click::Up)),
        Chunk::new("↓", Look::Reverse, Some(Click::Down)),
        Chunk::new(" Navigate  ", Look::Plain, Some(Click::Swap)),
        Chunk::new("Esc", Look::Reverse, Some(Click::Cancel)),
        Chunk::new(" Cancel  ", Look::Plain, Some(Click::Cancel)),
        Chunk::new("Enter", Look::Reverse, Some(Click::Apply)),
        Chunk::new(" Apply Changes", Look::Plain, Some(Click::Apply)),
    ]
}

fn narrow() -> Vec<Chunk> {
    vec![
        Chunk::new("Tab", Look::Reverse, Some(Click::Swap)),
        Chunk::new("↑", Look::Reverse, Some(Click::Up)),
        Chunk::new("↓", Look::Reverse, Some(Click::Down)),
        Chunk::new("  ", Look::Plain, None),
        Chunk::new("Esc", Look::Reverse, Some(Click::Cancel)),
        Chunk::new("  ", Look::Plain, None),
        Chunk::new("Enter", Look::Reverse, Some(Click::Apply)),
    ]
}

fn width(chunks: &[Chunk]) -> usize {
    chunks
        .iter()
        .map(|chunk| UnicodeWidthStr::width(chunk.text.as_str()))
        .sum()
}

fn hits_of(chunks: &[Chunk]) -> Vec<(usize, usize, Click)> {
    let mut hits = Vec::new();
    let mut x = 0usize;
    for chunk in chunks {
        let w = UnicodeWidthStr::width(chunk.text.as_str());
        if let Some(click) = chunk.hit {
            if let Some((_, end, last)) = hits.last_mut() {
                if *last == click && *end == x {
                    *end = x + w;
                    x += w;
                    continue;
                }
            }
            hits.push((x, x + w, click));
        }
        x += w;
    }
    hits
}

fn labeled(chunks: &[Chunk], click: Click) -> bool {
    chunks.iter().any(|chunk| {
        chunk.hit == Some(click)
            && chunk.look == Look::Plain
            && chunk.text.chars().any(|ch| ch != ' ')
    })
}

fn paint(chunks: &[Chunk], chrome: &Chrome, hover: Option<usize>) -> String {
    let active = hover.and_then(|column| {
        let mut x = 0usize;
        for chunk in chunks {
            let w = UnicodeWidthStr::width(chunk.text.as_str());
            if column >= x && column < x + w {
                return chunk.hit;
            }
            x += w;
        }
        None
    });
    let group = active.is_some_and(|click| labeled(chunks, click));
    let mut out = String::new();
    for chunk in chunks {
        out.push_str("\u{1b}[0m");
        let same = chunk.hit.is_some() && chunk.hit == active;
        let params = if same && group {
            format!("22;{};{}", chrome.dark.fg(), chrome.light.bg())
        } else {
            match (chunk.look, same) {
                (Look::Plain, false) => format!("22;{};{}", chrome.light.fg(), chrome.bar.bg()),
                (Look::Plain, true) => format!("22;{};{}", chrome.bar.fg(), chrome.light.bg()),
                (Look::Reverse, false) => format!("22;{};{}", chrome.dark.fg(), chrome.light.bg()),
                (Look::Reverse, true) => format!("22;{};{}", chrome.light.fg(), chrome.dark.bg()),
            }
        };
        out.push_str(&format!("\u{1b}[{params}m"));
        out.push_str(&chunk.text);
    }
    out.push_str("\u{1b}[0m");
    out
}

fn truncate(text: &str, cols: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    for character in text.chars() {
        let width = unicode_width::UnicodeWidthChar::width(character).unwrap_or(0);
        if used + width > cols {
            break;
        }
        out.push(character);
        used += width;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(rendered: &str) -> String {
        let mut out = String::new();
        let mut chars = rendered.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }
            out.push(ch);
        }
        out
    }

    #[test]
    fn wide_bar_names_every_control() {
        let (ansi, hits) = render(72, &Chrome::default(), None);
        let text = visible(&ansi);
        assert!(text.starts_with("Tab↑↓ Navigate  Esc Cancel  Enter Apply Changes"));
        assert_eq!(UnicodeWidthStr::width(text.as_str()), 72);
        assert_eq!(hit(72, &Chrome::default(), 0), Some(Click::Swap));
        assert_eq!(hit(72, &Chrome::default(), 3), Some(Click::Up));
        assert_eq!(hit(72, &Chrome::default(), 4), Some(Click::Down));
        assert_eq!(hit(72, &Chrome::default(), 6), Some(Click::Swap));
        assert_eq!(hit(72, &Chrome::default(), 16), Some(Click::Cancel));
        assert_eq!(hit(72, &Chrome::default(), 20), Some(Click::Cancel));
        assert_eq!(hit(72, &Chrome::default(), 28), Some(Click::Apply));
        assert_eq!(hit(72, &Chrome::default(), 34), Some(Click::Apply));
        assert_eq!(hits.iter().filter(|hit| hit.2 == Click::Swap).count(), 2);
    }

    #[test]
    fn narrow_bar_keeps_the_keys() {
        let (ansi, _) = render(30, &Chrome::default(), None);
        let text = visible(&ansi);
        assert!(text.starts_with("Tab↑↓  Esc  Enter"));
        assert!(!text.contains("Navigate"));
        assert_eq!(hit(30, &Chrome::default(), 0), Some(Click::Swap));
        assert_eq!(hit(30, &Chrome::default(), 7), Some(Click::Cancel));
        assert_eq!(hit(30, &Chrome::default(), 12), Some(Click::Apply));
    }

    #[test]
    fn hover_on_a_label_repaints_that_group() {
        let (plain, _) = render(72, &Chrome::default(), None);
        let (hovered, _) = render(72, &Chrome::default(), Some(6));
        assert_ne!(plain, hovered);
        let (arrow, _) = render(72, &Chrome::default(), Some(3));
        assert_ne!(arrow, hovered);
        assert_ne!(arrow, plain);
    }
}
