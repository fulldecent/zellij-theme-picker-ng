//! List, pinned footer, and checkbox rows.
//!
//! The list takes the rows left after the bar, the title, and the footer.
//! A short pane drops footer lines from the top and keeps the checkbox rows.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zellij_tile::ui_components::Text;

use crate::catalog::{slot_name, Baseline};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    List,
    Apply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxFocus {
    ConfigLight,
    ConfigDark,
    SessionLight,
    SessionDark,
}

impl BoxFocus {
    /// Up and down move through the four table rows. There is one box on each row.
    pub fn step(self, _dx: isize, dy: isize) -> Self {
        let index = match self {
            BoxFocus::SessionLight => 0,
            BoxFocus::SessionDark => 1,
            BoxFocus::ConfigLight => 2,
            BoxFocus::ConfigDark => 3,
        };
        match (index + dy).clamp(0, 3) {
            0 => BoxFocus::SessionLight,
            1 => BoxFocus::SessionDark,
            2 => BoxFocus::ConfigLight,
            _ => BoxFocus::ConfigDark,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplyBoxes {
    pub config_light: bool,
    pub config_dark: bool,
    pub session_light: bool,
    pub session_dark: bool,
}

impl ApplyBoxes {
    pub fn toggle(&mut self, focus: BoxFocus) {
        let slot = match focus {
            BoxFocus::ConfigLight => &mut self.config_light,
            BoxFocus::ConfigDark => &mut self.config_dark,
            BoxFocus::SessionLight => &mut self.session_light,
            BoxFocus::SessionDark => &mut self.session_dark,
        };
        *slot = !*slot;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterKind {
    /// Empty row directly above the connection sentence.
    Blank,
    Connection,
    SessionLight,
    SessionDark,
    ConfigLight,
    ConfigDark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FooterLine {
    pub y: usize,
    pub kind: FooterKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scrollbar {
    pub col: usize,
    /// First list row covered by the thumb, counted from the top of the list.
    pub thumb_start: usize,
    pub thumb_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub rows: usize,
    pub cols: usize,
    pub title: Option<usize>,
    pub list_y: usize,
    pub list_rows: usize,
    pub list_cols: usize,
    pub scrollbar: Option<Scrollbar>,
    pub footer: Vec<FooterLine>,
    pub bar_y: Option<usize>,
}

impl Frame {
    fn empty() -> Self {
        Self {
            rows: 0,
            cols: 0,
            title: None,
            list_y: 0,
            list_rows: 0,
            list_cols: 0,
            scrollbar: None,
            footer: Vec::new(),
            bar_y: None,
        }
    }
}

pub struct Line {
    pub x: usize,
    pub y: usize,
    pub text: Text,
}

pub struct View<'a> {
    pub names: &'a [String],
    pub selected: usize,
    pub scroll: usize,
    pub focus: Focus,
    pub box_focus: BoxFocus,
    pub boxes: ApplyBoxes,
    pub hue: Option<Hue>,
    pub notice: Option<&'a str>,
    pub live: &'a Baseline,
    pub baseline: &'a Baseline,
    pub rows: usize,
    pub cols: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    List(usize),
    ScrollUp,
    ScrollDown,
    Box(BoxFocus),
    Bar(crate::bar::Click),
}

/// Keep `selected` inside the visible window.
pub fn fit(selected: usize, scroll: usize, window: usize) -> usize {
    if window == 0 {
        return 0;
    }
    if selected < scroll {
        selected
    } else if selected >= scroll + window {
        selected + 1 - window
    } else {
        scroll
    }
}

pub fn frame(rows: usize, cols: usize, len: usize, scroll: usize) -> Frame {
    if rows == 0 || cols == 0 {
        return Frame::empty();
    }
    let bar_y = rows - 1;
    let above = rows - 1;
    // The four table rows stay. A short pane drops the blank, then the connection line.
    let table_rows = above.min(TABLE.len());
    let mut rest = above - table_rows;
    let has_title = rest > 0;
    if has_title {
        rest -= 1;
    }
    let upper = rest.min(UPPER.len());
    rest -= upper;
    let list_rows = rest;

    let mut footer = Vec::new();
    let footer_len = upper + table_rows;
    let mut y = bar_y - footer_len;
    for kind in UPPER.iter().skip(UPPER.len() - upper).copied() {
        footer.push(FooterLine { y, kind });
        y += 1;
    }
    for kind in TABLE.iter().skip(TABLE.len() - table_rows).copied() {
        footer.push(FooterLine { y, kind });
        y += 1;
    }

    let show_scroll = len > list_rows && list_rows > 0 && cols > 1;
    let scrollbar = if show_scroll {
        let (thumb_start, thumb_len) = thumb(list_rows, len, scroll);
        Some(Scrollbar {
            col: cols - 1,
            thumb_start,
            thumb_len,
        })
    } else {
        None
    };

    Frame {
        rows,
        cols,
        title: has_title.then_some(0),
        list_y: usize::from(has_title),
        list_rows,
        list_cols: if show_scroll { cols - 1 } else { cols },
        scrollbar,
        footer,
        bar_y: Some(bar_y),
    }
}

pub fn hit(frame: &Frame, line: isize, column: usize, len: usize, scroll: usize) -> Option<Hit> {
    if line < 0 {
        return None;
    }
    let line = line as usize;
    if let Some(bar) = frame.bar_y {
        if line == bar {
            return crate::bar::hit(frame.cols, &crate::bar::Chrome::default(), column)
                .map(Hit::Bar);
        }
    }
    if frame.list_rows > 0 && line >= frame.list_y && line < frame.list_y + frame.list_rows {
        let rel = line - frame.list_y;
        if let Some(bar) = frame.scrollbar {
            if column == bar.col {
                if rel < bar.thumb_start {
                    return Some(Hit::ScrollUp);
                }
                if rel >= bar.thumb_start + bar.thumb_len {
                    return Some(Hit::ScrollDown);
                }
                return None;
            }
        }
        if column < frame.list_cols {
            let index = scroll + rel;
            if index < len {
                return Some(Hit::List(index));
            }
        }
        return None;
    }
    let kind = frame.footer.iter().find(|item| item.y == line)?.kind;
    let focus = match kind {
        FooterKind::SessionLight => BoxFocus::SessionLight,
        FooterKind::SessionDark => BoxFocus::SessionDark,
        FooterKind::ConfigLight => BoxFocus::ConfigLight,
        FooterKind::ConfigDark => BoxFocus::ConfigDark,
        _ => return None,
    };
    let span = change_span(frame.cols);
    if span.contains(&column) && column < frame.cols {
        Some(Hit::Box(focus))
    } else {
        None
    }
}

pub fn paint(view: &View<'_>) -> Vec<Line> {
    let frame = frame(view.rows, view.cols, view.names.len(), view.scroll);
    let mut lines = Vec::new();
    if let Some(y) = frame.title {
        lines.push(plain(0, y, "Preview a theme:", frame.cols));
    }
    for offset in 0..frame.list_rows {
        let index = view.scroll + offset;
        let Some(name) = view.names.get(index) else {
            break;
        };
        let selected = index == view.selected;
        let marker = if selected { "> " } else { "  " };
        let body = pad(
            &truncate(&format!("{marker}{name}"), frame.list_cols),
            frame.list_cols,
        );
        let mut text = Text::new(body);
        if selected && view.focus == Focus::List {
            text = text.selected();
        }
        lines.push(Line {
            x: 0,
            y: frame.list_y + offset,
            text,
        });
    }
    if let Some(bar) = frame.scrollbar {
        for offset in 0..frame.list_rows {
            let mark = if offset >= bar.thumb_start && offset < bar.thumb_start + bar.thumb_len {
                "█"
            } else {
                "│"
            };
            lines.push(Line {
                x: bar.col,
                y: frame.list_y + offset,
                text: Text::new(mark),
            });
        }
    }
    for item in &frame.footer {
        match item.kind {
            FooterKind::Blank => {}
            FooterKind::Connection => {
                let text = connection_line(view.hue, view.notice);
                lines.push(plain(0, item.y, &text, frame.cols));
            }
            FooterKind::SessionLight => paint_table_row(
                &mut lines,
                item.y,
                frame.cols,
                "Session theme (light):",
                slot_name(&view.live.theme_light, &view.live.theme),
                view.boxes.session_light,
                view.focus == Focus::Apply && view.box_focus == BoxFocus::SessionLight,
            ),
            FooterKind::SessionDark => paint_table_row(
                &mut lines,
                item.y,
                frame.cols,
                "Session theme (dark):",
                slot_name(&view.live.theme_dark, &view.live.theme),
                view.boxes.session_dark,
                view.focus == Focus::Apply && view.box_focus == BoxFocus::SessionDark,
            ),
            FooterKind::ConfigLight => paint_table_row(
                &mut lines,
                item.y,
                frame.cols,
                "Config theme (light):",
                slot_name(&view.baseline.theme_light, &view.baseline.theme),
                view.boxes.config_light,
                view.focus == Focus::Apply && view.box_focus == BoxFocus::ConfigLight,
            ),
            FooterKind::ConfigDark => paint_table_row(
                &mut lines,
                item.y,
                frame.cols,
                "Config theme (dark):",
                slot_name(&view.baseline.theme_dark, &view.baseline.theme),
                view.boxes.config_dark,
                view.focus == Focus::Apply && view.box_focus == BoxFocus::ConfigDark,
            ),
        }
    }
    lines
}

const LABEL_WIDTH: usize = 22;
const NAME_WIDTH: usize = 24;
/// Blank, then the connection sentence. The blank is first so a short pane drops it first.
const UPPER: [FooterKind; 2] = [FooterKind::Blank, FooterKind::Connection];
const TABLE: [FooterKind; 4] = [
    FooterKind::SessionLight,
    FooterKind::SessionDark,
    FooterKind::ConfigLight,
    FooterKind::ConfigDark,
];

pub fn connection_line(hue: Option<Hue>, notice: Option<&str>) -> String {
    if let Some(notice) = notice {
        return notice.to_string();
    }
    match hue {
        Some(Hue::Light) => "Your connection is now set to light mode.".to_string(),
        Some(Hue::Dark) => "Your connection is now set to dark mode.".to_string(),
        None => "Your connection has not reported light or dark mode.".to_string(),
    }
}

/// `[+]` matches the PIN `[+]` experience. A plus marks the box that will change.
fn pin_mark(on: bool) -> &'static str {
    if on {
        "[+]"
    } else {
        "[ ]"
    }
}

fn name_width(cols: usize) -> usize {
    let fixed = LABEL_WIDTH + 2 + 2 + "[ ] change".len();
    cols.saturating_sub(fixed).min(NAME_WIDTH)
}

fn change_span(cols: usize) -> std::ops::Range<usize> {
    let start = LABEL_WIDTH + 2 + name_width(cols) + 2;
    start..start + "[ ] change".len()
}

fn table_line(label: &str, name: &str, on: bool, name_width: usize) -> String {
    let shown = pad(&truncate(name, name_width), name_width);
    format!("{label:<LABEL_WIDTH$}  {shown}  {} change", pin_mark(on))
}

fn paint_table_row(
    lines: &mut Vec<Line>,
    y: usize,
    cols: usize,
    label: &str,
    name: &str,
    on: bool,
    focused: bool,
) {
    let text = table_line(label, name, on, name_width(cols));
    let span = change_span(cols);
    let mark_end = span.end.min(text.chars().count());
    push_slice(lines, y, &text, 0..span.start, false, cols);
    if span.start < mark_end {
        push_slice(lines, y, &text, span.start..mark_end, focused, cols);
    }
}

fn push_slice(
    lines: &mut Vec<Line>,
    y: usize,
    text: &str,
    span: std::ops::Range<usize>,
    selected: bool,
    cols: usize,
) {
    if span.start >= cols || span.start >= span.end {
        return;
    }
    let end = span.end.min(cols).min(text.chars().count());
    if span.start >= end {
        return;
    }
    let slice: String = text
        .chars()
        .skip(span.start)
        .take(end - span.start)
        .collect();
    if slice.is_empty() {
        return;
    }
    let mut item = Text::new(slice);
    if selected {
        item = item.selected();
    }
    lines.push(Line {
        x: span.start,
        y,
        text: item,
    });
}

fn plain(x: usize, y: usize, text: &str, cols: usize) -> Line {
    Line {
        x,
        y,
        text: Text::new(truncate(text, cols.saturating_sub(x))),
    }
}

fn thumb(window: usize, len: usize, scroll: usize) -> (usize, usize) {
    let max_scroll = len.saturating_sub(window);
    let scroll = scroll.min(max_scroll);
    let thumb_len = (window.saturating_mul(window) / len.max(1))
        .max(1)
        .min(window);
    if max_scroll == 0 {
        return (0, window);
    }
    let travel = window.saturating_sub(thumb_len);
    let start = scroll.saturating_mul(travel) / max_scroll;
    (start.min(travel), thumb_len)
}

fn truncate(text: &str, cols: usize) -> String {
    if cols == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for character in text.chars() {
        let width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used + width > cols {
            break;
        }
        out.push(character);
        used += width;
    }
    out
}

fn pad(text: &str, cols: usize) -> String {
    let used = UnicodeWidthStr::width(text);
    let mut out = text.to_string();
    if used < cols {
        out.extend(std::iter::repeat_n(' ', cols - used));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("theme-{i}")).collect()
    }

    fn row_text(lines: &[Line], y: usize) -> String {
        let mut parts: Vec<_> = lines.iter().filter(|line| line.y == y).collect();
        parts.sort_by_key(|line| line.x);
        let mut out = String::new();
        for part in parts {
            let gap = part.x.saturating_sub(out.chars().count());
            out.extend(std::iter::repeat_n(' ', gap));
            out.push_str(part.text.content());
        }
        out
    }

    fn view<'a>(
        names: &'a [String],
        live: &'a Baseline,
        baseline: &'a Baseline,
        rows: usize,
        focus: Focus,
    ) -> View<'a> {
        View {
            names,
            selected: 1,
            scroll: 0,
            focus,
            box_focus: BoxFocus::SessionLight,
            boxes: ApplyBoxes::default(),
            hue: Some(Hue::Light),
            notice: None,
            live,
            baseline,
            rows,
            cols: 72,
        }
    }

    #[test]
    fn snapshot_geometry_pins_the_table_under_the_list() {
        let frame = frame(18, 72, 40, 0);
        assert_eq!(frame.title, Some(0));
        assert_eq!(frame.list_y, 1);
        assert_eq!(frame.list_rows, 10);
        assert_eq!(frame.bar_y, Some(17));
        assert_eq!(
            frame
                .footer
                .iter()
                .map(|item| item.kind)
                .collect::<Vec<_>>(),
            vec![
                FooterKind::Blank,
                FooterKind::Connection,
                FooterKind::SessionLight,
                FooterKind::SessionDark,
                FooterKind::ConfigLight,
                FooterKind::ConfigDark,
            ]
        );
        assert_eq!(frame.footer[0].y, 11);
        assert_eq!(frame.footer[5].y, 16);
        assert!(frame.scrollbar.is_some());
        assert_eq!(frame.list_cols, 71);
    }

    #[test]
    fn a_short_pane_drops_the_blank_line_and_keeps_the_table() {
        let frame = frame(7, 72, 40, 0);
        assert_eq!(frame.list_rows, 0);
        assert_eq!(frame.title, Some(0));
        assert_eq!(
            frame
                .footer
                .iter()
                .map(|item| item.kind)
                .collect::<Vec<_>>(),
            vec![
                FooterKind::Connection,
                FooterKind::SessionLight,
                FooterKind::SessionDark,
                FooterKind::ConfigLight,
                FooterKind::ConfigDark,
            ]
        );
        assert!(frame.scrollbar.is_none());
    }

    #[test]
    fn thumb_tracks_the_window() {
        let (start, len) = thumb(9, 40, 0);
        assert_eq!(len, 2);
        assert_eq!(start, 0);
        let (later, _) = thumb(9, 40, 31);
        assert_eq!(later, 7);
    }

    #[test]
    fn table_rows_share_the_change_column() {
        let width = name_width(72);
        let light = table_line("Session theme (light):", "nord", false, width);
        let dark = table_line("Session theme (dark):", "gruvbox-dark", true, width);
        let config = table_line("Config theme (light):", "pencil-light", false, width);
        assert_eq!(pin_mark(true), "[+]");
        assert_eq!(pin_mark(false), "[ ]");
        assert!(dark.contains("[+] change"));
        assert_eq!(light.find('['), dark.find('['));
        assert_eq!(light.find('['), config.find('['));
        assert_eq!(change_span(72).start, light.find('[').unwrap());
    }

    #[test]
    fn paint_puts_the_rest_of_the_pane_under_the_list() {
        let names = names(40);
        let live = Baseline {
            theme: Some("theme-1".to_string()),
            theme_light: Some("theme-1".to_string()),
            theme_dark: Some("theme-1".to_string()),
        };
        let baseline = Baseline {
            theme: Some("nord".to_string()),
            theme_light: Some("pencil-light".to_string()),
            ..Baseline::default()
        };
        let lines = paint(&view(&names, &live, &baseline, 18, Focus::List));
        assert_eq!(lines[0].text.content(), "Preview a theme:");
        let selected = lines
            .iter()
            .find(|line| line.text.content().starts_with("> theme-1"))
            .expect("selected row");
        assert_eq!(selected.y, 2);
        assert!(selected.text.serialize().starts_with('x'));
        assert_eq!(UnicodeWidthStr::width(selected.text.content()), 71);
        let plain = lines
            .iter()
            .find(|line| line.text.content().starts_with("  theme-0"))
            .expect("other row");
        assert!(!plain.text.serialize().starts_with('x'));
        assert!(
            lines.iter().all(|line| line.y != 11),
            "blank row stays empty"
        );
        assert_eq!(
            row_text(&lines, 12),
            "Your connection is now set to light mode."
        );
        let width = name_width(72);
        assert_eq!(
            row_text(&lines, 13),
            table_line("Session theme (light):", "theme-1", false, width)
        );
        assert_eq!(
            row_text(&lines, 14),
            table_line("Session theme (dark):", "theme-1", false, width)
        );
        assert_eq!(
            row_text(&lines, 15),
            table_line("Config theme (light):", "pencil-light", false, width)
        );
        assert_eq!(
            row_text(&lines, 16),
            table_line("Config theme (dark):", "nord", false, width)
        );
        let marks: String = lines
            .iter()
            .filter(|line| line.x == 71)
            .map(|line| line.text.content().to_string())
            .collect();
        assert_eq!(marks, "██││││││││");
    }

    #[test]
    fn list_highlight_waits_until_the_list_has_focus() {
        let names = names(4);
        let baseline = Baseline::default();
        let lines = paint(&view(&names, &baseline, &baseline, 18, Focus::Apply));
        let selected = lines
            .iter()
            .find(|line| line.text.content().starts_with("> theme-1"))
            .expect("marker");
        assert!(!selected.text.serialize().starts_with('x'));
        let focused = lines
            .iter()
            .find(|line| line.text.content() == "[ ] change")
            .expect("session light");
        assert!(focused.text.serialize().starts_with('x'));
    }

    #[test]
    fn mouse_hits_follow_the_same_rows() {
        let geometry = frame(18, 72, 40, 0);
        assert_eq!(hit(&geometry, 2, 3, 40, 0), Some(Hit::List(1)));
        // At the top, the thumb covers the first rows. A later scroll leaves a click above it.
        let lowered = frame(18, 72, 40, 15);
        assert_eq!(hit(&lowered, 1, 71, 40, 15), Some(Hit::ScrollUp));
        assert_eq!(hit(&geometry, 4, 71, 40, 0), Some(Hit::ScrollDown));
        assert_eq!(hit(&geometry, 1, 71, 40, 0), None);
        let change = change_span(72).start;
        assert_eq!(
            hit(&geometry, 13, change, 40, 0),
            Some(Hit::Box(BoxFocus::SessionLight))
        );
        assert_eq!(
            hit(&geometry, 15, change, 40, 0),
            Some(Hit::Box(BoxFocus::ConfigLight))
        );
        assert_eq!(
            hit(&geometry, 16, change, 40, 0),
            Some(Hit::Box(BoxFocus::ConfigDark))
        );
        assert_eq!(hit(&geometry, 15, 19, 40, 0), None);
        assert_eq!(
            hit(&geometry, 17, 0, 40, 0),
            Some(Hit::Bar(crate::bar::Click::Swap))
        );
        assert_eq!(hit(&geometry, -1, 0, 40, 0), None);
    }

    #[test]
    fn fit_keeps_the_selection_on_screen() {
        assert_eq!(fit(0, 0, 4), 0);
        assert_eq!(fit(5, 0, 4), 2);
        assert_eq!(fit(1, 3, 4), 1);
        assert_eq!(fit(4, 2, 4), 2);
    }

    #[test]
    fn notice_replaces_the_connection_sentence() {
        assert_eq!(
            connection_line(None, Some("Could not write config.kdl.")),
            "Could not write config.kdl."
        );
        assert_eq!(
            connection_line(None, None),
            "Your connection has not reported light or dark mode."
        );
        assert_eq!(
            connection_line(Some(Hue::Dark), None),
            "Your connection is now set to dark mode."
        );
    }
}
