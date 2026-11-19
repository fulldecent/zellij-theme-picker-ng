//! Theme names and the `config.kdl` edit.
//!
//! Each theme name is its own row. `gruvbox-dark` and `gruvbox-light` stay separate.

use std::collections::BTreeSet;

/// Theme keys read from the root of a config file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Baseline {
    pub theme: Option<String>,
    pub theme_dark: Option<String>,
    pub theme_light: Option<String>,
}

/// Root keys Enter may write. `None` leaves that line in the file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigPatch {
    pub theme_dark: Option<String>,
    pub theme_light: Option<String>,
}

/// Names found while reading a KDL file, plus the root theme keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub baseline: Baseline,
    pub theme_names: Vec<String>,
}

const BUILTIN: &[&str] = &[
    "ansi",
    "ao",
    "atelier-sulphurpool",
    "ayu-dark",
    "ayu-light",
    "ayu-mirage",
    "blade-runner",
    "catppuccin-frappe",
    "catppuccin-latte",
    "catppuccin-macchiato",
    "catppuccin-mocha",
    "cyber-noir",
    "dayfox",
    "default",
    "dracula",
    "everforest-dark",
    "everforest-light",
    "flexoki-dark",
    "gruber-darker",
    "gruvbox-dark",
    "gruvbox-light",
    "iceberg-dark",
    "iceberg-light",
    "kanagawa",
    "lucario",
    "menace",
    "molokai-dark",
    "night-owl",
    "nightfox",
    "nord",
    "one-half-dark",
    "onedark",
    "pencil-light",
    "retro-wave",
    "solarized-dark",
    "solarized-light",
    "terafox",
    "tokyo-night",
    "tokyo-night-dark",
    "tokyo-night-light",
    "tokyo-night-storm",
    "vesper",
];

/// Built-in theme names shipped with Zellij 0.45.1, plus the code name `default`.
///
/// `atelier.kdl` defines `atelier-sulphurpool`. `default-light` is not in this version.
pub fn builtin() -> Vec<String> {
    BUILTIN.iter().map(|name| (*name).to_string()).collect()
}

/// Sorted unique names. Empty names and control characters are dropped.
pub fn unique_names<I, S>(names: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut unique = BTreeSet::new();
    for name in names {
        let name = name.as_ref();
        if kdl_quote(name).is_some() {
            unique.insert(name.to_string());
        }
    }
    unique.into_iter().collect()
}

/// Preview sets every session slot to `name`, so the palette shows in either hue.
///
/// `explicit_theme_hue` is left unset. A merged config cannot clear it.
pub fn preview_kdl(name: &str) -> Option<String> {
    let quoted = kdl_quote(name)?;
    Some(format!(
        "theme {quoted}\ntheme_dark {quoted}\ntheme_light {quoted}\n"
    ))
}

/// Session keys after Enter.
///
/// A checked slot becomes `name`. An unchecked slot returns to the baseline.
/// The static `theme` key returns to the baseline so a preview cannot stick.
/// An unset `theme` becomes `default`. An unset dark or light slot becomes `""`.
pub fn merged_kdl(baseline: &Baseline, name: &str, light: bool, dark: bool) -> Option<String> {
    let theme = match baseline.theme.as_deref() {
        Some(value) if !value.is_empty() => kdl_quote(value)?,
        _ => "\"default\"".to_string(),
    };
    let theme_dark = if dark {
        kdl_quote(name)?
    } else {
        quoted_slot(baseline.theme_dark.as_deref())?
    };
    let theme_light = if light {
        kdl_quote(name)?
    } else {
        quoted_slot(baseline.theme_light.as_deref())?
    };
    Some(format!(
        "theme {theme}\ntheme_dark {theme_dark}\ntheme_light {theme_light}\n"
    ))
}

fn quoted_slot(value: Option<&str>) -> Option<String> {
    match value {
        Some(name) if !name.is_empty() => kdl_quote(name),
        _ => Some("\"\"".to_string()),
    }
}

/// Quote a theme name for a KDL string. Empty names and control characters are rejected.
pub fn kdl_quote(name: &str) -> Option<String> {
    if name.is_empty() || name.chars().any(char::is_control) {
        return None;
    }
    let mut quoted = String::from("\"");
    for character in name.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    Some(quoted)
}

/// The name shown for one slot. An empty slot uses the static `theme`, then `unset`.
pub fn slot_name<'a>(slot: &'a Option<String>, theme: &'a Option<String>) -> &'a str {
    if let Some(name) = nonempty(slot) {
        return name;
    }
    if let Some(name) = nonempty(theme) {
        return name;
    }
    "unset"
}

fn nonempty(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|name| !name.is_empty())
}

/// KDL that puts the session back. Unset dark and light slots become empty strings.
/// An unset `theme` becomes `default`. This text is applied with `save` false.
pub fn restore_kdl(baseline: &Baseline) -> Option<String> {
    let theme = match baseline.theme.as_deref() {
        Some(name) if !name.is_empty() => kdl_quote(name)?,
        _ => "\"default\"".to_string(),
    };
    let theme_dark = quoted_slot(baseline.theme_dark.as_deref())?;
    let theme_light = quoted_slot(baseline.theme_light.as_deref())?;
    Some(format!(
        "theme {theme}\ntheme_dark {theme_dark}\ntheme_light {theme_light}\n"
    ))
}

/// Prefer the static `theme`, then `theme_light`, then `theme_dark`.
pub fn locate(names: &[String], baseline: &Baseline) -> usize {
    for candidate in [
        baseline.theme.as_deref(),
        baseline.theme_light.as_deref(),
        baseline.theme_dark.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_empty() {
            continue;
        }
        if let Some(index) = names.iter().position(|name| name == candidate) {
            return index;
        }
    }
    0
}

pub fn scan_kdl(text: &str) -> Scan {
    let tokens = tokenize(text);
    let mut scan = Scan::default();
    let mut index = 0usize;
    let mut depth = 0i32;
    let mut themes_depth: Option<i32> = None;

    while index < tokens.len() {
        match &tokens[index] {
            Tok::LBrace => {
                depth += 1;
                index += 1;
            }
            Tok::RBrace => {
                if themes_depth == Some(depth) {
                    themes_depth = None;
                }
                depth = depth.saturating_sub(1);
                index += 1;
            }
            Tok::String(_) => index += 1,
            Tok::Ident(name) => {
                let followed_by_brace = matches!(tokens.get(index + 1), Some(Tok::LBrace));
                let followed_by_string = match tokens.get(index + 1) {
                    Some(Tok::String(value)) => Some(value),
                    _ => None,
                };
                if depth == 0 {
                    if let Some(value) = followed_by_string {
                        let recorded = match name.as_str() {
                            "theme_dark" => {
                                scan.baseline.theme_dark = Some(value.clone());
                                true
                            }
                            "theme_light" => {
                                scan.baseline.theme_light = Some(value.clone());
                                true
                            }
                            "theme" => {
                                scan.baseline.theme = Some(value.clone());
                                true
                            }
                            // `theme_dir` shares a prefix with `theme` and is not a theme choice.
                            "theme_dir" => true,
                            _ => false,
                        };
                        if recorded {
                            index += 2;
                            continue;
                        }
                    }
                    if name == "themes" && followed_by_brace {
                        depth = 1;
                        themes_depth = Some(depth);
                        index += 2;
                        continue;
                    }
                } else if themes_depth == Some(depth)
                    && followed_by_brace
                    && kdl_quote(name).is_some()
                {
                    scan.theme_names.push(name.clone());
                }
                if followed_by_brace {
                    depth += 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        }
    }
    scan
}

/// Replace the checked root `theme_dark` and `theme_light` lines and keep every other line.
///
/// The static `theme` line stays. `reconfigure(..., true)` rewrites the whole file and
/// drops comments, so callers that already read `config.kdl` write this text back instead.
pub fn config_with_patch(original: &str, patch: &ConfigPatch) -> Option<String> {
    let mut body = String::new();
    if let Some(name) = &patch.theme_dark {
        body.push_str("theme_dark ");
        body.push_str(&kdl_quote(name)?);
        body.push('\n');
    }
    if let Some(name) = &patch.theme_light {
        body.push_str("theme_light ");
        body.push_str(&kdl_quote(name)?);
        body.push('\n');
    }
    if body.is_empty() {
        return Some(original.to_string());
    }
    let mut out = String::new();
    let mut depth = 0i32;
    let mut in_block = false;
    for line in original.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        let (delta, still_in_block) = brace_delta(content, in_block);
        let trimmed = content.trim();
        let drop = depth == 0
            && !in_block
            && ((patch.theme_dark.is_some() && is_root_key(trimmed, "theme_dark"))
                || (patch.theme_light.is_some() && is_root_key(trimmed, "theme_light")));
        if !drop {
            out.push_str(line);
        }
        depth += delta;
        in_block = still_in_block;
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&body);
    Some(out)
}

/// `theme_dark` and `theme_light` share a prefix with `theme`. A following `_` is a different key.
fn is_root_key(trimmed: &str, key: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix(key) else {
        return false;
    };
    matches!(
        rest.chars().next(),
        Some(character) if character.is_whitespace() || character == '"' || character == ';'
    )
}

fn brace_delta(line: &str, mut in_block: bool) -> (i32, bool) {
    let chars: Vec<char> = line.chars().collect();
    let mut delta = 0i32;
    let mut index = 0;
    while index < chars.len() {
        if in_block {
            if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                in_block = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
            break;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
            in_block = true;
            index += 2;
            continue;
        }
        if chars[index] == '"' {
            index += 1;
            while index < chars.len() {
                if chars[index] == '\\' {
                    index = index.saturating_add(2);
                    continue;
                }
                if chars[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }
        if chars[index] == '{' {
            delta += 1;
        } else if chars[index] == '}' {
            delta -= 1;
        }
        index += 1;
    }
    (delta, in_block)
}

enum Tok {
    Ident(String),
    String(String),
    LBrace,
    RBrace,
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut tokens = Vec::new();
    let mut parser = Parser { text, index: 0 };
    while parser.index < parser.text.len() {
        if parser.starts_with("//") {
            parser.index += 2;
            while let Some(character) = parser.bump() {
                if character == '\n' {
                    break;
                }
            }
            continue;
        }
        if parser.starts_with("/*") {
            parser.index += 2;
            loop {
                if parser.rest().is_empty() {
                    break;
                }
                if parser.starts_with("*/") {
                    parser.index += 2;
                    break;
                }
                parser.bump();
            }
            continue;
        }
        if parser
            .rest()
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            parser.bump();
            continue;
        }
        if parser.starts_with("{") {
            parser.index += 1;
            tokens.push(Tok::LBrace);
            continue;
        }
        if parser.starts_with("}") {
            parser.index += 1;
            tokens.push(Tok::RBrace);
            continue;
        }
        if parser.starts_with("\"") {
            if let Some(value) = parser.read_string() {
                tokens.push(Tok::String(value));
            }
            continue;
        }
        if parser
            .rest()
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        {
            let mut name = String::new();
            while let Some(character) = parser.rest().chars().next() {
                if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                    name.push(character);
                    parser.bump();
                } else {
                    break;
                }
            }
            tokens.push(Tok::Ident(name));
            continue;
        }
        parser.bump();
    }
    tokens
}

struct Parser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.index..]
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.rest().starts_with(prefix)
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.rest().chars().next()?;
        self.index += character.len_utf8();
        Some(character)
    }

    fn read_string(&mut self) -> Option<String> {
        if self.bump() != Some('"') {
            return None;
        }
        let mut value = String::new();
        while let Some(character) = self.bump() {
            match character {
                '"' => return Some(value),
                '\\' => match self.bump() {
                    Some('\\') => value.push('\\'),
                    Some('"') => value.push('"'),
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('r') => value.push('\r'),
                    Some(other) => value.push(other),
                    None => return None,
                },
                _ => value.push(character),
            }
        }
        None
    }
}

pub fn move_index(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = len - 1;
    if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_separate_and_sorted() {
        let names = unique_names([
            "nord",
            "gruvbox-light",
            "gruvbox-dark",
            "gruvbox-dark",
            "",
            "bad\nname",
        ]);
        assert_eq!(
            names,
            vec![
                "gruvbox-dark".to_string(),
                "gruvbox-light".to_string(),
                "nord".to_string(),
            ]
        );
    }

    #[test]
    fn builtins_keep_both_hues_and_default() {
        let names = unique_names(builtin());
        for name in [
            "ayu-dark",
            "ayu-light",
            "gruvbox-dark",
            "gruvbox-light",
            "gruber-darker",
            "default",
            "atelier-sulphurpool",
            "tokyo-night",
            "tokyo-night-dark",
            "tokyo-night-light",
        ] {
            assert!(names.iter().any(|item| item == name), "{name}");
        }
        assert!(!names.iter().any(|item| item == "default-light"));
    }

    #[test]
    fn preview_sets_every_slot_to_the_highlighted_name() {
        assert_eq!(
            preview_kdl("nord").as_deref(),
            Some("theme \"nord\"\ntheme_dark \"nord\"\ntheme_light \"nord\"\n")
        );
    }

    #[test]
    fn merged_kdl_checks_only_the_requested_slots() {
        let baseline = Baseline {
            theme: Some("default".to_string()),
            theme_dark: Some("gruvbox-dark".to_string()),
            theme_light: Some("gruvbox-light".to_string()),
        };
        assert_eq!(
            merged_kdl(&baseline, "nord", true, false).as_deref(),
            Some("theme \"default\"\ntheme_dark \"gruvbox-dark\"\ntheme_light \"nord\"\n")
        );
        assert_eq!(
            merged_kdl(&Baseline::default(), "nord", false, true).as_deref(),
            Some("theme \"default\"\ntheme_dark \"nord\"\ntheme_light \"\"\n")
        );
    }

    #[test]
    fn quotes_reject_empty_and_controls_and_escape_the_rest() {
        assert!(kdl_quote("").is_none());
        assert!(kdl_quote("a\nb").is_none());
        assert_eq!(kdl_quote(r#"a"b\c"#).as_deref(), Some(r#""a\"b\\c""#));
    }

    #[test]
    fn slot_name_falls_back_to_the_static_theme() {
        let theme = Some("nord".to_string());
        assert_eq!(
            slot_name(&Some("ayu-light".to_string()), &theme),
            "ayu-light"
        );
        assert_eq!(slot_name(&None, &theme), "nord");
        assert_eq!(slot_name(&Some(String::new()), &None), "unset");
    }

    #[test]
    fn scan_ignores_comments_nested_blocks_and_theme_dir() {
        let text = r#"
            // theme "nope"
            theme "nord"
            /* theme_dark "nope" */
            theme_dark "gruvbox-dark"
            theme_light "gruvbox-light"
            theme_dir "/tmp/themes"
            keybinds {
                theme "hidden"
            }
            themes {
                mine {
                    bg 1 2 3
                }
                nested {
                    inner {
                    }
                }
            }
            theme "a\"b\\c"
        "#;
        let found = scan_kdl(text);
        assert_eq!(found.baseline.theme.as_deref(), Some(r#"a"b\c"#));
        assert_eq!(found.baseline.theme_dark.as_deref(), Some("gruvbox-dark"));
        assert_eq!(found.baseline.theme_light.as_deref(), Some("gruvbox-light"));
        assert_eq!(
            found.theme_names,
            vec!["mine".to_string(), "nested".to_string()]
        );
    }

    #[test]
    fn scan_does_not_treat_theme_dark_as_theme() {
        let found = scan_kdl("theme_dark \"only-dark\"\n");
        assert_eq!(found.baseline.theme, None);
        assert_eq!(found.baseline.theme_dark.as_deref(), Some("only-dark"));
    }

    #[test]
    fn patch_replaces_only_the_checked_root_keys() {
        let original = r#"theme "old"
theme_dark "old-dark"
theme_light "old-light"
theme_dir "themes"
themes {
    mine {
        bg 1 2 3
    }
}
keybinds {
    theme "hidden"
}
"#;
        let next = config_with_patch(
            original,
            &ConfigPatch {
                theme_light: Some("nord".to_string()),
                theme_dark: None,
            },
        )
        .expect("quoted");
        assert!(next.contains("theme \"old\"\n"));
        assert!(next.contains("theme_dark \"old-dark\""));
        assert!(!next.contains("old-light"));
        assert!(next.contains("theme_dir \"themes\""));
        assert!(next.contains("mine {"));
        assert!(next.contains("theme \"hidden\""));
        assert!(next.ends_with("theme_light \"nord\"\n"));
    }

    #[test]
    fn locate_prefers_the_static_theme_then_light_then_dark() {
        let names = unique_names(["default", "gruvbox-dark", "gruvbox-light", "nord"]);
        let theme = Baseline {
            theme: Some("nord".to_string()),
            theme_dark: Some("gruvbox-dark".to_string()),
            theme_light: Some("gruvbox-light".to_string()),
        };
        assert_eq!(names[locate(&names, &theme)], "nord");
        let light = Baseline {
            theme_light: Some("gruvbox-light".to_string()),
            theme_dark: Some("gruvbox-dark".to_string()),
            ..Baseline::default()
        };
        assert_eq!(names[locate(&names, &light)], "gruvbox-light");
        let dark = Baseline {
            theme_dark: Some("gruvbox-dark".to_string()),
            ..Baseline::default()
        };
        assert_eq!(names[locate(&names, &dark)], "gruvbox-dark");
        assert_eq!(locate(&names, &Baseline::default()), 0);
    }

    #[test]
    fn restore_uses_default_and_empty_slots_when_unset() {
        assert_eq!(
            restore_kdl(&Baseline::default()).as_deref(),
            Some("theme \"default\"\ntheme_dark \"\"\ntheme_light \"\"\n")
        );
    }

    #[test]
    fn movement_stays_in_range() {
        assert_eq!(move_index(0, -1, 4), 0);
        assert_eq!(move_index(3, 5, 4), 3);
        assert_eq!(move_index(1, -1, 4), 0);
    }
}
