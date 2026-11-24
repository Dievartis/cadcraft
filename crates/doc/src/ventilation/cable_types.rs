//! How a cable is written: its number of conductors, whether one is an earth, whether it is shielded, and its section ("3G1.5", "5G2.5", "2x0.75 blindé"),
//! and the library of cable types the user picks from or saves for another cable: a starting list, then the drawing's own.

use serde::{Deserialize, Serialize};

use super::CableUsage;

/// The text written after a shielded cable.
pub const SHIELDED_TEXT: &str = "blindé";
/// Most conductors a cable has, and most cable types a drawing keeps.
pub const MAX_CORES: u32 = 100;
pub const MAX_CABLE_TYPES: usize = 256;

/// A cable as it is written: `cores` conductors of `section_mm2`, one of them an earth (the G of 3G1.5; an X, as in 2x0.75, has none), shielded or not.
/// `cores` is `None` for a section alone ("16"): the conductors are then those of the circuit's phases (3G for a single-phase circuit, 5G for a three-phase one).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableSpec {
    #[serde(default)]
    pub cores: Option<u32>,
    #[serde(default)]
    pub earth: bool,
    #[serde(default)]
    pub shielded: bool,
    pub section_mm2: f64,
}

/// A number as a section is written: "1.5", "0.75", "16".
fn number_text(v: f64) -> String {
    let t = format!("{v:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl CableSpec {
    /// The cable as it is written: "3G2.5", "2x0.75 blindé", "16" for a section alone.
    pub fn text(&self) -> String {
        let mut t = match self.cores {
            Some(n) => format!("{n}{}{}", if self.earth { 'G' } else { 'x' }, number_text(self.section_mm2)),
            None => number_text(self.section_mm2),
        };
        if self.shielded {
            t.push(' ');
            t.push_str(SHIELDED_TEXT);
        }
        t
    }

    pub fn validate(&self) -> bool {
        self.section_mm2.is_finite()
            && (0.05..=630.0).contains(&self.section_mm2)
            && self.cores.is_none_or(|n| (1..=MAX_CORES).contains(&n))
            && (self.cores.is_some() || !self.earth)
    }
}

/// Reads a cable as it is typed: "3G1.5", "5g2,5 mm²", "2x0.75", "2 X 0.75 mm2 blindé", "4x1.5 shielded", "16". The number of conductors comes first, then G (with an
/// earth) or x (without one), then the section in mm²; "blindé", "blindée", "écran", "shielded", "FTP"… after it (or before) mark a shielded cable. `None` when
/// the text is not a cable.
pub fn parse_cable_spec(text: &str) -> Option<CableSpec> {
    let lower = text.trim().to_lowercase().replace(',', ".");
    // The shield, as a word anywhere in the text.
    let shield_words = ["blind", "ecran", "écran", "shield", "screen", "ftp", "stp", "s/ftp", "u/ftp"];
    let shielded = shield_words.iter().any(|w| lower.contains(w));
    // What is left: the digits, the letter and the units.
    let core: String =
        lower.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace("mm²", "").replace("mm2", "").replace("mm", "").replace('²', "");
    // Drop the words of the shield (and the letters that follow the number in them).
    let numeric: String = core.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | 'g' | 'x' | '×')).collect();
    if numeric.is_empty() {
        return None;
    }
    // The rest of the text must be the shield's words or nothing.
    let rest = core[numeric.len()..].to_string();
    if !rest.is_empty() && !shield_words.iter().any(|w| rest.contains(w)) {
        return None;
    }
    let spec = match numeric.find(['g', 'x', '×']) {
        Some(at) => {
            let (n, tail) = numeric.split_at(at);
            let earth = tail.starts_with('g');
            let section = tail[tail.chars().next()?.len_utf8()..].parse::<f64>().ok()?;
            CableSpec { cores: Some(n.parse::<u32>().ok()?), earth, shielded, section_mm2: section }
        }
        None => CableSpec { cores: None, earth: false, shielded, section_mm2: numeric.parse::<f64>().ok()? },
    };
    spec.validate().then_some(spec)
}

/// A named cable: what it is for (its use, when it has one) and how it is written.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CableType {
    pub name: String,
    #[serde(default)]
    pub usage: Option<CableUsage>,
    pub spec: CableSpec,
}

impl CableType {
    pub fn validate(&self) -> bool {
        let name = self.name.trim();
        !name.is_empty() && name.chars().count() <= 60 && !name.chars().any(char::is_control) && self.spec.validate()
    }
}

/// The cable types the library starts with, by use: the usual sections of power and lighting cables, the shielded pairs of the buses and the data cables, and the
/// cables of remote controls and safety circuits.
pub fn standard_cable_types() -> Vec<CableType> {
    let mut v = Vec::new();
    let mut add = |name: String, usage: CableUsage, cores: u32, earth: bool, shielded: bool, section: f64| {
        v.push(CableType { name, usage: Some(usage), spec: CableSpec { cores: Some(cores), earth, shielded, section_mm2: section } });
    };
    for s in [1.5, 2.5, 4.0, 6.0, 10.0, 16.0] {
        add(format!("Power 3G{}", number_text(s)), CableUsage::Power, 3, true, false, s);
    }
    for s in [1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0] {
        add(format!("Power 5G{}", number_text(s)), CableUsage::Power, 5, true, false, s);
    }
    add("Lighting 3G1.5".into(), CableUsage::Lighting, 3, true, false, 1.5);
    add("Lighting 5G1.5".into(), CableUsage::Lighting, 5, true, false, 1.5);
    add("Safety 2x1.5 (CR1)".into(), CableUsage::Safety, 2, false, false, 1.5);
    add("Safety 3G1.5 (CR1)".into(), CableUsage::Safety, 3, true, false, 1.5);
    add("Data cat. 6 shielded, 4 pairs".into(), CableUsage::Data, 8, false, true, 0.25);
    add("Data cat. 6A shielded, 4 pairs".into(), CableUsage::Data, 8, false, true, 0.3);
    for (name, cores, section) in
        [("1 pair 2x0.22", 2, 0.22), ("1 pair 2x0.5", 2, 0.5), ("1 pair 2x0.75", 2, 0.75), ("2 pairs 4x0.22", 4, 0.22), ("2 pairs 4x0.5", 4, 0.5)]
    {
        add(format!("Bus {name} shielded"), CableUsage::Bus, cores, false, true, section);
    }
    for (name, cores, section, shielded) in
        [("2x0.75", 2, 0.75, false), ("2x0.75 shielded", 2, 0.75, true), ("2x1 shielded", 2, 1.0, true), ("4x0.75 shielded", 4, 0.75, true)]
    {
        add(format!("Remote control {name}"), CableUsage::Remote, cores, false, shielded, section);
    }
    v
}

impl crate::Drawing {
    /// Every cable type a cable can take: the drawing's, then the starting ones it does not replace (a drawing type of the same name wins).
    pub fn available_cable_types(&self) -> Vec<CableType> {
        let mut v = self.cable_types.clone();
        for t in standard_cable_types() {
            if !v.iter().any(|d| d.name.eq_ignore_ascii_case(&t.name)) {
                v.push(t);
            }
        }
        v
    }

    /// The cable type called `name` (case does not matter).
    pub fn cable_type_named(&self, name: &str) -> Option<CableType> {
        self.available_cable_types().into_iter().find(|t| t.name.eq_ignore_ascii_case(name.trim()))
    }

    /// Keeps `t` in the drawing's library, replacing the one of the same name.
    pub fn save_cable_type(&mut self, mut t: CableType) -> crate::Result<()> {
        t.name = t.name.trim().to_string();
        if !t.validate() {
            return Err(super::invalid("A cable type has a name and a cable as 3G1.5 or 2x0.75."));
        }
        if let Some(slot) = self.cable_types.iter_mut().find(|d| d.name.eq_ignore_ascii_case(&t.name)) {
            *slot = t;
        } else if self.cable_types.len() >= MAX_CABLE_TYPES {
            return Err(super::invalid("The drawing keeps at most 256 cable types."));
        } else {
            self.cable_types.push(t);
        }
        Ok(())
    }

    /// Takes cable type `name` out of the drawing's library; whether there was one.
    pub fn delete_cable_type(&mut self, name: &str) -> bool {
        let before = self.cable_types.len();
        self.cable_types.retain(|t| !t.name.eq_ignore_ascii_case(name.trim()));
        self.cable_types.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cables_are_read_as_they_are_typed() {
        let spec = |c: Option<u32>, earth: bool, shielded: bool, s: f64| Some(CableSpec { cores: c, earth, shielded, section_mm2: s });
        assert_eq!(parse_cable_spec("3G1.5"), spec(Some(3), true, false, 1.5));
        assert_eq!(parse_cable_spec("3g1,5 mm²"), spec(Some(3), true, false, 1.5));
        assert_eq!(parse_cable_spec("5G2.5mm2"), spec(Some(5), true, false, 2.5));
        assert_eq!(parse_cable_spec("2x0.75"), spec(Some(2), false, false, 0.75));
        assert_eq!(parse_cable_spec("2 X 0,75 mm² blindé"), spec(Some(2), false, true, 0.75));
        assert_eq!(parse_cable_spec("4x1.5 blindée"), spec(Some(4), false, true, 1.5));
        assert_eq!(parse_cable_spec("2x0.75mm² shielded"), spec(Some(2), false, true, 0.75));
        assert_eq!(parse_cable_spec("8x0.25 S/FTP"), spec(Some(8), false, true, 0.25));
        assert_eq!(parse_cable_spec("16"), spec(None, false, false, 16.0));
        assert_eq!(parse_cable_spec(" 35 mm² "), spec(None, false, false, 35.0));
        for bad in ["", "abc", "3G", "xG1.5", "3G0", "0x1.5", "101x1.5", "3G1.5 gris", "3G1.5.5", "1e9", "-4", "3G700"] {
            assert_eq!(parse_cable_spec(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_cable_is_written_back_as_it_was_read() {
        for text in ["3G1.5", "5G2.5", "2x0.75 blindé", "8x0.25 blindé", "16", "3G0.75", "12x1.5"] {
            assert_eq!(parse_cable_spec(text).unwrap().text(), text, "{text}");
        }
        assert_eq!(parse_cable_spec("2x0,75mm2 shielded").unwrap().text(), "2x0.75 blindé");
    }

    #[test]
    fn the_starting_list_is_valid_unique_and_covers_every_use() {
        let types = standard_cable_types();
        let mut names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), types.len(), "names are unique");
        assert!(types.iter().all(CableType::validate));
        for u in CableUsage::ALL {
            assert!(types.iter().any(|t| t.usage == Some(u)), "{u:?} has a starting cable");
        }
        assert!(types.iter().any(|t| t.spec.text() == "3G1.5") && types.iter().any(|t| t.spec.text() == "2x0.75 blindé"));
    }

    #[test]
    fn a_drawing_keeps_its_own_cable_types_which_win_over_the_starting_ones() {
        let mut d = crate::Drawing::new_metric();
        let n = d.available_cable_types().len();
        let mine = CableType { name: "Bus KNX".into(), usage: Some(CableUsage::Bus), spec: parse_cable_spec("2x0.8 blindé").unwrap() };
        d.save_cable_type(mine.clone()).unwrap();
        assert_eq!(d.available_cable_types().len(), n + 1);
        assert_eq!(d.cable_type_named("bus knx"), Some(mine));
        // The same name replaces it, a starting name is shadowed, a bad type is refused, and it can be deleted.
        d.save_cable_type(CableType { name: "BUS KNX".into(), usage: None, spec: parse_cable_spec("2x1").unwrap() }).unwrap();
        assert_eq!(d.cable_types.len(), 1);
        d.save_cable_type(CableType { name: "power 3g1.5".into(), usage: None, spec: parse_cable_spec("3G1.5 blindé").unwrap() }).unwrap();
        assert_eq!(d.available_cable_types().len(), n + 2 - 1, "the one of that name is replaced, not added");
        assert!(d.cable_type_named("Power 3G1.5").unwrap().spec.shielded);
        assert!(d.save_cable_type(CableType { name: " ".into(), usage: None, spec: parse_cable_spec("3G1.5").unwrap() }).is_err());
        assert!(d.delete_cable_type("bus knx") && !d.delete_cable_type("bus knx"));
    }
}
