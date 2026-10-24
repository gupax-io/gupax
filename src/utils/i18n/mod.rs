// Gupax
//
// Copyright (c) 2024-2025 Cyrix126
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// Lightweight multi-language i18n framework: UI strings are written in English
// (source of truth) and looked up in the per-language tables when another
// language is selected. Missing entries gracefully fall back to English.
//
// This build ships the framework only (no bundled translations or fonts).
// To add a language:
//   1. Add a variant to [Language] and fill in [Language::native_name],
//      [Language::code] and (optionally) [Language::font].
//   2. Create `xx.rs` next to this file with:
//        pub static PAIRS: &[(&str, &str)] = &[("english source", "translation"), ...];
//      The english side must match the UI string exactly.
//   3. Register the table in [TABLES].
//   4. Optional: add a font in [Language::font] and install it in [install_fonts].
//   5. Run `cargo test`: placeholder counts and table sanity are checked.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use strum::{EnumCount, EnumIter};

/// User interface language, persisted in [Gupax] state.
#[derive(
    Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash, EnumIter, EnumCount,
)]
pub enum Language {
    #[default]
    English,
}

impl Language {
    /// The language name shown in the language selector (in that language).
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
        }
    }
    /// Short language code, used in logs and file names.
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
        }
    }
    /// Language used when a key is missing from this language's table.
    pub const fn fallback(self) -> Option<Language> {
        None
    }
    /// Font required to render this language (appended as a glyph fallback).
    /// Example once a CJK language is added:
    /// `Self::SimplifiedChinese => Some(BYTES_NOTO_SC),`
    pub const fn font(self) -> Option<&'static [u8]> {
        match self {
            Self::English => None,
        }
    }
}

/// Per-language translation tables: (english source, translation).
/// English is the source language and has no table.
static TABLES: &[(Language, &[(&str, &str)])] = &[(Language::English, &[])];

static TABLE: Lazy<HashMap<Language, HashMap<&'static str, &'static str>>> = Lazy::new(|| {
    TABLES
        .iter()
        .map(|(language, pairs)| (*language, pairs.iter().copied().collect()))
        .collect()
});

static CURRENT: AtomicU8 = AtomicU8::new(Language::English as u8);

pub fn set_language(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        code if code == Language::English as u8 => Language::English,
        _ => Language::English,
    }
}

/// Whether more than one language is registered (the selector is shown only
/// when there is something to choose).
pub fn has_language_choice() -> bool {
    TABLES.len() > 1
}

/// Look up an english key in `language`, following the fallback chain.
fn lookup(language: Language, en: &str) -> Option<&'static str> {
    let mut current = Some(language);
    while let Some(language) = current {
        if let Some(translation) = TABLE.get(&language).and_then(|table| table.get(en)) {
            return Some(translation);
        }
        current = language.fallback();
    }
    None
}

/// Translate an English string at runtime.
/// Missing translations fall back to the English source string.
pub fn tr(en: &'static str) -> &'static str {
    lookup(language(), en).unwrap_or(en)
}

/// Translate a borrowed English string (for runtime-built labels).
/// Missing translations fall back to the English source string.
pub fn tr_ref(en: &str) -> &str {
    match lookup(language(), en) {
        Some(translation) => translation,
        None => en,
    }
}

/// Apply the fonts needed by the current language on top of egui's defaults.
/// This build bundles no fonts; a language that needs one should append it
/// here (see [Language::font]).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let families = [egui::FontFamily::Proportional, egui::FontFamily::Monospace];
    if let Some(bytes) = language().font() {
        fonts.font_data.insert(
            "noto_lang".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
        for family in families {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("noto_lang".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

// Static translation lookup: [t!("english")] -> translated string or english.
macro_rules! t {
    ($s:expr) => {
        $crate::utils::i18n::tr($s)
    };
}
pub(crate) use t;

// Template translation with format arguments:
// [t_f!("found {} nodes", n)] -> translated template then formatted.
// Supports plain "{}" placeholders (translated templates are runtime strings,
// so [format!] cannot be used directly on them).
macro_rules! t_f {
    ($s:expr $(, $arg:expr)*) => {
        $crate::utils::i18n::tr_format($s, &[$(format!("{}", $arg)),*])
    };
}
pub(crate) use t_f;

/// Fill `{}` placeholders of a (possibly translated) template in order.
pub fn tr_format(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut parts = template.split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        match args.get(i) {
            Some(arg) => out.push_str(arg),
            None => out.push_str("{}"),
        }
        out.push_str(part);
    }
    out
}

//---------------------------------------------------------------------------------------------------- TESTS
#[cfg(test)]
mod test {
    use super::*;
    use strum::IntoEnumIterator;

    // These tests never change the global language: other test modules
    // (e.g. `human.rs`) assert English output and run in parallel.

    #[test]
    fn english_is_passthrough() {
        assert_eq!(tr("P2Pool is offline"), "P2Pool is offline");
    }

    #[test]
    fn english_needs_no_table() {
        assert_eq!(lookup(Language::English, "P2Pool is offline"), None);
    }

    #[test]
    fn tr_format_fills_placeholders() {
        let args = vec!["42".to_string(), "P2Pool".to_string()];
        assert_eq!(tr_format("{} x {}", &args), "42 x P2Pool");
        assert_eq!(tr_format("{} {}", &["a".to_string()]), "a {}");
        assert_eq!(tr_format("plain", &args), "plain");
    }

    #[test]
    fn every_language_has_a_table() {
        for language in Language::iter() {
            if language == Language::English {
                continue;
            }
            assert!(
                TABLES
                    .iter()
                    .any(|(table_language, _)| *table_language == language),
                "missing translation table for {language:?}"
            );
        }
    }

    #[test]
    fn tables_are_well_formed() {
        for (language, pairs) in TABLES {
            let mut seen = HashMap::new();
            for (en, translation) in *pairs {
                assert!(!en.is_empty(), "{language:?}: empty english key");
                assert!(
                    !translation.is_empty(),
                    "{language:?}: empty translation of {en}"
                );
                assert_ne!(
                    en, translation,
                    "{language:?}: untranslated entry left in table: {en}"
                );
                assert_eq!(
                    en.matches("{}").count(),
                    translation.matches("{}").count(),
                    "{language:?}: placeholder mismatch for: {en}"
                );
                if let Some(previous) = seen.insert(*en, *translation) {
                    assert_eq!(
                        previous, *translation,
                        "{language:?}: conflicting duplicate entry: {en}"
                    );
                }
            }
        }
    }

    #[test]
    fn selector_is_hidden_with_a_single_language() {
        assert!(!has_language_choice());
    }
}
