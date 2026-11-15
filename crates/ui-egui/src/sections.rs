//! The sections of the Architecture and HVAC tabs that fold: which are open when the tab is shown. The choice (Options, Interface) is to open
//! them all, to close them all, or to remember how each was left the last time (kept between runs with the other preferences). Whatever the
//! choice, a section can be folded or opened at any time; the choice decides how the program starts and is applied at once when it is changed.

use std::collections::HashMap;

use serde_json::{Value, json};

/// How the sections are when the program starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SectionsMode {
    Open,
    Closed,
    /// Each section as it was left the last time.
    #[default]
    Remember,
}

impl SectionsMode {
    pub const ALL: [SectionsMode; 3] = [SectionsMode::Open, SectionsMode::Closed, SectionsMode::Remember];

    pub fn as_str(self) -> &'static str {
        match self {
            SectionsMode::Open => "open",
            SectionsMode::Closed => "closed",
            SectionsMode::Remember => "remember",
        }
    }

    pub fn parse(s: &str) -> Option<SectionsMode> {
        SectionsMode::ALL.into_iter().find(|m| m.as_str().eq_ignore_ascii_case(s.trim()))
    }
}

/// Most sections whose state is kept.
const MAX_KEPT: usize = 256;

/// The state of the sections: the choice, what was kept from the last runs, and what was done in this one.
#[derive(Clone, Debug, Default)]
pub struct Sections {
    pub mode: SectionsMode,
    /// The state of each section, kept between runs (only changed while the choice is to remember).
    kept: HashMap<String, bool>,
    /// What the user folded or opened since the program started or the choice changed: it wins over the choice.
    session: HashMap<String, bool>,
}

impl Sections {
    /// Whether the section called `key` is open.
    pub fn is_open(&self, key: &str) -> bool {
        self.session.get(key).copied().unwrap_or_else(|| match self.mode {
            SectionsMode::Open => true,
            SectionsMode::Closed => false,
            SectionsMode::Remember => self.kept.get(key).copied().unwrap_or(true),
        })
    }

    /// The user folded or opened section `key`.
    pub fn set(&mut self, key: &str, open: bool) {
        self.session.insert(key.to_string(), open);
        if self.mode == SectionsMode::Remember && (self.kept.len() < MAX_KEPT || self.kept.contains_key(key)) {
            self.kept.insert(key.to_string(), open);
        }
    }

    /// Changes the choice and applies it to every section at once.
    pub fn set_mode(&mut self, mode: SectionsMode) {
        self.mode = mode;
        self.session.clear();
    }

    /// The preferences to keep, as the host stores them.
    pub fn prefs(&self) -> Value {
        json!({ "mode": self.mode.as_str(), "states": self.kept })
    }

    /// Restores [`Self::prefs`]; what is not understood is ignored.
    pub fn load_prefs(&mut self, v: &Value) {
        if let Some(m) = v.get("mode").and_then(Value::as_str).and_then(SectionsMode::parse) {
            self.mode = m;
        }
        if let Some(states) = v.get("states").and_then(Value::as_object) {
            self.kept = states.iter().filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b))).take(MAX_KEPT).collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_decides_how_the_sections_are_and_a_click_wins_for_the_run() {
        let mut s = Sections::default();
        assert_eq!(s.mode, SectionsMode::Remember);
        assert!(s.is_open("HVAC/Ducts"), "nothing kept yet: open");
        s.set("HVAC/Ducts", false);
        assert!(!s.is_open("HVAC/Ducts") && s.is_open("HVAC/Spaces"));
        // Closed: all closed at once, then one opened by a click; Open: all open.
        s.set_mode(SectionsMode::Closed);
        assert!(!s.is_open("HVAC/Ducts") && !s.is_open("HVAC/Spaces"));
        s.set("HVAC/Spaces", true);
        assert!(s.is_open("HVAC/Spaces") && !s.is_open("HVAC/Ducts"));
        s.set_mode(SectionsMode::Open);
        assert!(s.is_open("HVAC/Ducts") && s.is_open("HVAC/Spaces"));
    }

    #[test]
    fn only_the_choice_to_remember_keeps_how_each_section_was_left() {
        let mut s = Sections::default();
        s.set("A/x", false);
        s.set("A/y", true);
        let saved = s.prefs();
        let mut next = Sections::default();
        next.load_prefs(&saved);
        assert_eq!((next.mode, next.is_open("A/x"), next.is_open("A/y"), next.is_open("A/z")), (SectionsMode::Remember, false, true, true));
        // Not remembering: what is done in a run is not kept, and the saved states are not used.
        let mut closed = Sections::default();
        closed.load_prefs(&saved);
        closed.set_mode(SectionsMode::Closed);
        closed.set("A/z", true);
        let after = closed.prefs();
        assert_eq!(after["mode"], "closed");
        assert!(after["states"].get("A/z").is_none(), "{after}");
        let mut reopened = Sections::default();
        reopened.load_prefs(&after);
        assert_eq!((reopened.mode, reopened.is_open("A/x"), reopened.is_open("A/z")), (SectionsMode::Closed, false, false));
        // Malformed preferences are ignored.
        let mut junk = Sections::default();
        junk.load_prefs(&json!({"mode": 7, "states": {"a": "no", "b": true}}));
        assert_eq!((junk.mode, junk.is_open("a"), junk.is_open("b")), (SectionsMode::Remember, true, true));
        assert_eq!(SectionsMode::parse(" Closed "), Some(SectionsMode::Closed));
        assert_eq!(SectionsMode::parse("nope"), None);
    }
}
