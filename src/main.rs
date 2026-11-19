use std::io::{self, Write};
use std::path::PathBuf;

use zellij_tile::prelude::*;
use zellij_tile::ui_components::print_text_with_coordinates;

use bar::{chrome_from, Chrome, Click};
use catalog::{
    config_with_patch, kdl_quote, locate, merged_kdl, move_index, preview_kdl, restore_kdl,
    scan_kdl, unique_names, Baseline, ConfigPatch,
};
use render::{fit, frame, hit, paint, ApplyBoxes, BoxFocus, Focus, Frame, Hit, Hue, View};

mod bar;
mod catalog;
mod render;

struct State {
    names: Vec<String>,
    selected: usize,
    scroll: usize,
    focus: Focus,
    box_focus: BoxFocus,
    boxes: ApplyBoxes,
    hue: Option<Hue>,
    baseline: Baseline,
    live: Baseline,
    baseline_known: bool,
    can_reconfigure: bool,
    config_text: Option<String>,
    user_moved: bool,
    /// The highlighted name last sent as a preview. A repeat move skips the call.
    last_preview: Option<String>,
    /// The session no longer matches the baseline captured when the file was read.
    session_dirty: bool,
    waiting_save: bool,
    notice: Option<String>,
    rows: usize,
    cols: usize,
    chrome: Chrome,
    bar_hover: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            names: unique_names(catalog::builtin()),
            selected: 0,
            scroll: 0,
            focus: Focus::List,
            box_focus: BoxFocus::SessionLight,
            boxes: ApplyBoxes::default(),
            hue: None,
            baseline: Baseline::default(),
            live: Baseline::default(),
            baseline_known: false,
            can_reconfigure: false,
            config_text: None,
            user_moved: false,
            last_preview: None,
            session_dirty: false,
            waiting_save: false,
            notice: None,
            rows: 0,
            cols: 0,
            chrome: Chrome::default(),
            bar_hover: None,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: std::collections::BTreeMap<String, String>) {
        request_permission(&[PermissionType::Reconfigure, PermissionType::FullHdAccess]);
        subscribe(&[
            EventType::Key,
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::HostTerminalThemeChanged,
            EventType::PermissionRequestResult,
            EventType::HostFolderChanged,
            EventType::FailedToChangeHostFolder,
            EventType::FailedToWriteConfigToDisk,
            EventType::ConfigWasWrittenToDisk,
        ]);
        set_selectable(true);
        // The list is the control. A cursor would sit on top of the highlighted row.
        show_cursor(None);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                self.can_reconfigure = true;
                self.clear_permission_notice();
                match config_dir() {
                    Some(dir) => change_host_folder(dir),
                    None => {
                        self.notice =
                            Some("HOME is unset, so the config folder stayed closed.".to_string());
                    }
                }
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.can_reconfigure = false;
                true
            }
            Event::HostFolderChanged(_) => {
                self.read_host();
                true
            }
            Event::FailedToChangeHostFolder(_) => false,
            Event::ConfigWasWrittenToDisk => {
                if self.waiting_save {
                    close_self();
                }
                false
            }
            Event::FailedToWriteConfigToDisk(_) if self.waiting_save => {
                self.waiting_save = false;
                self.notice = Some("Could not write the config file.".to_string());
                true
            }
            Event::ModeUpdate(info) => {
                let next = chrome_from(&info.style);
                if next == self.chrome {
                    false
                } else {
                    self.chrome = next;
                    true
                }
            }
            Event::HostTerminalThemeChanged(mode) => {
                let next = match mode {
                    HostTerminalThemeMode::Light => Hue::Light,
                    HostTerminalThemeMode::Dark => Hue::Dark,
                };
                if self.hue == Some(next) {
                    false
                } else {
                    self.hue = Some(next);
                    true
                }
            }
            Event::Key(key) => self.on_key(key),
            Event::Mouse(mouse) => self.on_mouse(mouse),
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.rows = rows;
        self.cols = cols;
        if rows == 0 || cols == 0 {
            return;
        }
        self.refit();
        let lines = paint(&View {
            names: &self.names,
            selected: self.selected,
            scroll: self.scroll,
            focus: self.focus,
            box_focus: self.box_focus,
            boxes: self.boxes,
            hue: self.hue,
            notice: self.notice.as_deref(),
            live: &self.live,
            baseline: &self.baseline,
            rows,
            cols,
        });
        let geometry = self.geometry();
        // A full-width row wraps onto the next line unless autowrap is off.
        print!("\u{1b}[?7l");
        for line in lines {
            print_text_with_coordinates(line.text, line.x, line.y, Some(cols), None);
        }
        if let Some(y) = geometry.bar_y {
            let (ansi, _) = bar::render(cols, &self.chrome, self.bar_hover);
            // Cursor addresses are 1-based. The bar is the last row of the pane.
            print!("\u{1b}[{};1H{ansi}", y + 1);
        }
        print!("\u{1b}[?7h");
        // Plugin stdout is block-buffered. Zellij reads it when render returns.
        let _ = io::stdout().flush();
    }
}

impl State {
    fn on_key(&mut self, key: KeyWithModifier) -> bool {
        if !key.has_no_modifiers() || (self.waiting_save && !matches!(key.bare_key, BareKey::Esc)) {
            return false;
        }
        let page = self.geometry().list_rows.max(1) as isize;
        match key.bare_key {
            BareKey::Esc => {
                self.dismiss();
                false
            }
            BareKey::Enter => self.commit(),
            BareKey::Tab => {
                self.focus = match self.focus {
                    Focus::List => Focus::Apply,
                    Focus::Apply => Focus::List,
                };
                true
            }
            BareKey::Up | BareKey::Char('k') => self.nudge(-1),
            BareKey::Down | BareKey::Char('j') => self.nudge(1),
            BareKey::Left | BareKey::Char('h') => self.nudge_across(-1),
            BareKey::Right | BareKey::Char('l') => self.nudge_across(1),
            BareKey::Char(' ') => self.toggle_focused_box(),
            BareKey::PageUp => self.page(-page),
            BareKey::PageDown => self.page(page),
            BareKey::Home => self.jump_list(0),
            BareKey::End => self.jump_list(self.names.len().saturating_sub(1)),
            _ => false,
        }
    }

    fn on_mouse(&mut self, mouse: Mouse) -> bool {
        if self.waiting_save {
            return false;
        }
        match mouse {
            Mouse::ScrollUp(lines) => self.move_list(-(lines as isize)),
            Mouse::ScrollDown(lines) => self.move_list(lines as isize),
            Mouse::Hover(line, column) => self.hover(line, column),
            Mouse::LeftClick(line, column) => self.click(line, column),
            _ => false,
        }
    }

    fn hover(&mut self, line: isize, column: usize) -> bool {
        let geometry = self.geometry();
        if line >= 0 && geometry.bar_y == Some(line as usize) {
            let next = Some(column);
            if self.bar_hover == next {
                return false;
            }
            self.bar_hover = next;
            return true;
        }
        let cleared = self.bar_hover.take().is_some();
        match hit(&geometry, line, column, self.names.len(), self.scroll) {
            Some(Hit::List(index)) => self.point_at(index) || cleared,
            _ => cleared,
        }
    }

    fn click(&mut self, line: isize, column: usize) -> bool {
        let geometry = self.geometry();
        let page = geometry.list_rows.max(1) as isize;
        match hit(&geometry, line, column, self.names.len(), self.scroll) {
            Some(Hit::List(index)) => self.point_at(index),
            Some(Hit::ScrollUp) => self.move_list(-page),
            Some(Hit::ScrollDown) => self.move_list(page),
            Some(Hit::Box(focus)) => {
                self.focus = Focus::Apply;
                self.box_focus = focus;
                self.boxes.toggle(focus);
                true
            }
            Some(Hit::Bar(click)) => self.bar_click(click),
            None => false,
        }
    }

    fn bar_click(&mut self, click: Click) -> bool {
        match click {
            Click::Swap => {
                self.focus = match self.focus {
                    Focus::List => Focus::Apply,
                    Focus::Apply => Focus::List,
                };
                true
            }
            Click::Up => self.nudge(-1),
            Click::Down => self.nudge(1),
            Click::Cancel => {
                self.dismiss();
                false
            }
            Click::Apply => self.commit(),
        }
    }

    fn nudge(&mut self, delta: isize) -> bool {
        match self.focus {
            Focus::List => self.move_list(delta),
            Focus::Apply => {
                let next = self.box_focus.step(0, delta);
                if next == self.box_focus {
                    return false;
                }
                self.box_focus = next;
                true
            }
        }
    }

    fn nudge_across(&mut self, delta: isize) -> bool {
        if self.focus != Focus::Apply {
            return false;
        }
        let next = self.box_focus.step(delta, 0);
        if next == self.box_focus {
            return false;
        }
        self.box_focus = next;
        true
    }

    fn toggle_focused_box(&mut self) -> bool {
        if self.focus != Focus::Apply {
            return false;
        }
        self.boxes.toggle(self.box_focus);
        true
    }

    fn page(&mut self, delta: isize) -> bool {
        if self.focus != Focus::List {
            return false;
        }
        self.move_list(delta)
    }

    fn jump_list(&mut self, index: usize) -> bool {
        if self.focus != Focus::List {
            return false;
        }
        self.point_at(index)
    }

    fn move_list(&mut self, delta: isize) -> bool {
        let next = move_index(self.selected, delta, self.names.len());
        if next == self.selected {
            return false;
        }
        self.selected = next;
        self.user_moved = true;
        self.refit();
        self.push_preview();
        true
    }

    fn point_at(&mut self, index: usize) -> bool {
        if self.names.is_empty() {
            return false;
        }
        let index = index.min(self.names.len() - 1);
        let moved = index != self.selected;
        let focus_changed = self.focus != Focus::List;
        if !moved && !focus_changed {
            return false;
        }
        self.focus = Focus::List;
        self.selected = index;
        if moved {
            self.user_moved = true;
            self.refit();
            self.push_preview();
        }
        true
    }

    fn push_preview(&mut self) {
        let Some(name) = self.names.get(self.selected).cloned() else {
            return;
        };
        if self.last_preview.as_deref() == Some(name.as_str()) {
            return;
        }
        let Some(kdl) = preview_kdl(&name) else {
            self.notice = Some("That theme name cannot be shown.".to_string());
            return;
        };
        if !self.can_reconfigure {
            self.notice = Some("Zellij did not allow changing the theme.".to_string());
            return;
        }
        reconfigure(kdl, false);
        self.last_preview = Some(name.clone());
        self.session_dirty = true;
        self.live = Baseline {
            theme: Some(name.clone()),
            theme_dark: Some(name.clone()),
            theme_light: Some(name),
        };
        self.clear_permission_notice();
    }

    fn commit(&mut self) -> bool {
        if self.waiting_save {
            return false;
        }
        let config_on = self.boxes.config_light || self.boxes.config_dark;
        let session_on = self.boxes.session_light || self.boxes.session_dark;
        let name = self.names.get(self.selected).cloned();
        if config_on || session_on {
            let Some(name) = name.as_deref() else {
                self.notice = Some("There is no theme to apply.".to_string());
                return true;
            };
            if kdl_quote(name).is_none() {
                self.notice = Some("That theme name cannot be saved.".to_string());
                return true;
            }
        }
        if !self.settle_session(session_on, name.as_deref()) {
            return true;
        }
        if !config_on {
            close_self();
            return false;
        }
        let Some(name) = name.as_deref() else {
            self.notice = Some("There is no theme to apply.".to_string());
            return true;
        };
        if let Some(text) = self.config_text.as_deref() {
            let patch = ConfigPatch {
                theme_light: self.boxes.config_light.then(|| name.to_string()),
                theme_dark: self.boxes.config_dark.then(|| name.to_string()),
            };
            let Some(next) = config_with_patch(text, &patch) else {
                self.notice = Some("That theme name cannot be saved.".to_string());
                return true;
            };
            return match write_config(&next) {
                Ok(()) => {
                    close_self();
                    false
                }
                Err(_) => {
                    self.notice = Some("Could not write config.kdl.".to_string());
                    true
                }
            };
        }
        // The file was not read, so there is no text to patch. Zellij rewrites the file.
        if !self.can_reconfigure {
            self.notice = Some("Zellij did not allow changing the theme.".to_string());
            return true;
        }
        let Some(kdl) = merged_kdl(
            &self.baseline_for_restore(),
            name,
            self.boxes.config_light,
            self.boxes.config_dark,
        ) else {
            self.notice = Some("That theme name cannot be saved.".to_string());
            return true;
        };
        reconfigure(kdl, true);
        self.waiting_save = true;
        self.notice = Some("Saving the config file.".to_string());
        true
    }

    /// Put the session where Enter asked. Returns false when the pane should stay open.
    fn settle_session(&mut self, session_on: bool, name: Option<&str>) -> bool {
        if !session_on && !self.session_dirty {
            return true;
        }
        if !self.can_reconfigure {
            self.notice = Some("Zellij did not allow changing the theme.".to_string());
            return false;
        }
        let baseline = self.baseline_for_restore();
        if session_on {
            let Some(name) = name else {
                self.notice = Some("There is no theme to apply.".to_string());
                return false;
            };
            let Some(kdl) = merged_kdl(
                &baseline,
                name,
                self.boxes.session_light,
                self.boxes.session_dark,
            ) else {
                self.notice = Some("That theme name cannot be saved.".to_string());
                return false;
            };
            reconfigure(kdl, false);
            self.live = Baseline {
                theme: baseline.theme.clone(),
                theme_light: if self.boxes.session_light {
                    Some(name.to_string())
                } else {
                    baseline.theme_light.clone()
                },
                theme_dark: if self.boxes.session_dark {
                    Some(name.to_string())
                } else {
                    baseline.theme_dark.clone()
                },
            };
            // Not a full three-slot preview, so the next highlight sends one again.
            self.last_preview = None;
            self.session_dirty = true;
            return true;
        }
        let Some(kdl) = restore_kdl(&baseline) else {
            self.notice = Some("That theme name cannot be saved.".to_string());
            return false;
        };
        reconfigure(kdl, false);
        self.live = baseline;
        self.last_preview = None;
        self.session_dirty = false;
        true
    }

    fn dismiss(&mut self) {
        self.waiting_save = false;
        if self.session_dirty && self.can_reconfigure {
            let baseline = self.baseline_for_restore();
            if let Some(kdl) = restore_kdl(&baseline) {
                reconfigure(kdl, false);
            }
        }
        close_self();
    }

    fn baseline_for_restore(&self) -> Baseline {
        if self.baseline_known {
            self.baseline.clone()
        } else {
            Baseline::default()
        }
    }

    fn clear_permission_notice(&mut self) {
        if self.notice.as_deref() == Some("Zellij did not allow changing the theme.") {
            self.notice = None;
        }
    }

    fn read_host(&mut self) {
        let mut found = catalog::builtin();
        match std::fs::read_to_string("/host/config.kdl") {
            Ok(text) => {
                let scan = scan_kdl(&text);
                found.extend(scan.theme_names);
                self.baseline = scan.baseline.clone();
                self.baseline_known = true;
                self.config_text = Some(text);
                if !self.user_moved {
                    self.live = scan.baseline;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.baseline_known = false;
                self.config_text = None;
            }
            Err(_) => {
                self.baseline_known = false;
                self.config_text = None;
                self.notice = Some("Could not read config.kdl.".to_string());
            }
        }
        read_theme_files(&mut found);
        self.install_names(found);
    }

    fn install_names(&mut self, found: Vec<String>) {
        let previous = self.names.get(self.selected).cloned();
        self.names = unique_names(found);
        if self.names.is_empty() {
            self.selected = 0;
            return;
        }
        if !self.user_moved && self.baseline_known {
            self.selected = locate(&self.names, &self.baseline);
            self.live = self.baseline.clone();
        } else if let Some(previous) = previous {
            if let Some(index) = self.names.iter().position(|name| name == &previous) {
                self.selected = index;
            } else {
                self.selected = self.selected.min(self.names.len() - 1);
            }
        } else {
            self.selected = self.selected.min(self.names.len() - 1);
        }
        self.refit();
    }

    fn geometry(&self) -> Frame {
        frame(self.rows, self.cols, self.names.len(), self.scroll)
    }

    fn refit(&mut self) {
        let window = frame(self.rows, self.cols, self.names.len(), 0).list_rows;
        self.scroll = fit(self.selected, self.scroll, window);
    }
}

fn config_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    if home.is_empty() {
        return None;
    }
    // Zellij 0.45.1 uses `$HOME/.config/zellij` on Unix, not `XDG_CONFIG_HOME`.
    Some(PathBuf::from(home).join(".config").join("zellij"))
}

fn read_theme_files(names: &mut Vec<String>) {
    let Ok(directory) = std::fs::read_dir("/host/themes") else {
        return;
    };
    for entry in directory.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("kdl") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        names.extend(scan_kdl(&text).theme_names);
    }
}

fn write_config(text: &str) -> std::io::Result<()> {
    let temporary = std::path::Path::new("/host/config.kdl.tmp");
    let destination = std::path::Path::new("/host/config.kdl");
    std::fs::write(temporary, text)?;
    if std::fs::rename(temporary, destination).is_err() {
        std::fs::write(destination, text)?;
        let _ = std::fs::remove_file(temporary);
    }
    Ok(())
}
