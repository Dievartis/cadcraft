//! Rolled steel sections: the dimensions and the mass per metre of the common beams and columns (IPN, IPE,
//! HEA, HEB and UPN), for the structural objects of a plan.
//!
//! The figures are the nominal ones of the published standards: DIN 1025-1 for the IPN (the flanges' thickness is
//! the mean one, they are tapered) and EN 10365 / EN 10034 for the IPE, HEA, HEB and UPN. They are entered here by
//! hand as design figures, not taken from a manufacturer's catalogue: check them against the one of the supplier
//! before ordering. Dimensions in mm, mass in kg/m.

/// A family of rolled sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SectionFamily {
    /// Standard I beam, tapered flanges (DIN 1025-1).
    Ipn,
    /// European I beam, parallel flanges.
    Ipe,
    /// Wide-flange beam, light series.
    Hea,
    /// Wide-flange beam, standard series.
    Heb,
    /// Channel, tapered flanges.
    Upn,
}

impl SectionFamily {
    pub const ALL: [SectionFamily; 5] = [SectionFamily::Ipn, SectionFamily::Ipe, SectionFamily::Hea, SectionFamily::Heb, SectionFamily::Upn];

    pub fn name(self) -> &'static str {
        match self {
            SectionFamily::Ipn => "IPN",
            SectionFamily::Ipe => "IPE",
            SectionFamily::Hea => "HEA",
            SectionFamily::Heb => "HEB",
            SectionFamily::Upn => "UPN",
        }
    }

    /// The family a name designates ("ipn", "HEA 200" → HEA), whatever follows the letters.
    pub fn parse(text: &str) -> Option<SectionFamily> {
        let letters: String = text.trim().chars().take_while(|c| c.is_ascii_alphabetic()).collect::<String>().to_ascii_uppercase();
        SectionFamily::ALL.into_iter().find(|f| f.name() == letters)
    }

    /// Whether the section is a channel (open on one side) rather than an I.
    pub fn is_channel(self) -> bool {
        self == SectionFamily::Upn
    }
}

/// One rolled section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteelSection {
    pub family: SectionFamily,
    /// The number of the designation ("IPN 200" is 200): the height for the IPN, IPE and UPN, the nominal one for the HEA and HEB.
    pub number: u32,
    /// The height h, mm.
    pub h_mm: f64,
    /// The flange width b, mm.
    pub b_mm: f64,
    /// The web thickness tw, mm.
    pub tw_mm: f64,
    /// The flange thickness tf, mm.
    pub tf_mm: f64,
    /// The mass, kg/m.
    pub kg_m: f64,
}

impl SteelSection {
    /// The designation: "IPN 200".
    pub fn designation(&self) -> String {
        format!("{} {}", self.family.name(), self.number)
    }

    /// The cross-section area from the drawn shape, mm² (the fillets are ignored: the mass table is the reference).
    pub fn shape_area_mm2(&self) -> f64 {
        2.0 * self.b_mm * self.tf_mm + (self.h_mm - 2.0 * self.tf_mm) * self.tw_mm
    }
}

const fn s(family: SectionFamily, number: u32, h: f64, b: f64, tw: f64, tf: f64, kg: f64) -> SteelSection {
    SteelSection { family, number, h_mm: h, b_mm: b, tw_mm: tw, tf_mm: tf, kg_m: kg }
}

use SectionFamily::{Hea, Heb, Ipe, Ipn, Upn};

/// Every section of the table, by family then size.
pub const STEEL_SECTIONS: &[SteelSection] = &[
    // IPN (DIN 1025-1).
    s(Ipn, 80, 80.0, 42.0, 3.9, 5.9, 5.94),
    s(Ipn, 100, 100.0, 50.0, 4.5, 6.8, 8.34),
    s(Ipn, 120, 120.0, 58.0, 5.1, 7.7, 11.1),
    s(Ipn, 140, 140.0, 66.0, 5.7, 8.6, 14.3),
    s(Ipn, 160, 160.0, 74.0, 6.3, 9.5, 17.9),
    s(Ipn, 180, 180.0, 82.0, 6.9, 10.4, 21.9),
    s(Ipn, 200, 200.0, 90.0, 7.5, 11.3, 26.2),
    s(Ipn, 220, 220.0, 98.0, 8.1, 12.2, 31.1),
    s(Ipn, 240, 240.0, 106.0, 8.7, 13.1, 36.2),
    s(Ipn, 260, 260.0, 113.0, 9.4, 14.1, 41.9),
    s(Ipn, 280, 280.0, 119.0, 10.1, 15.2, 47.9),
    s(Ipn, 300, 300.0, 125.0, 10.8, 16.2, 54.2),
    s(Ipn, 320, 320.0, 131.0, 11.5, 17.3, 61.1),
    s(Ipn, 340, 340.0, 137.0, 12.2, 18.3, 68.0),
    s(Ipn, 360, 360.0, 143.0, 13.0, 19.5, 76.1),
    s(Ipn, 380, 380.0, 149.0, 13.7, 20.5, 84.0),
    s(Ipn, 400, 400.0, 155.0, 14.4, 21.6, 92.6),
    // IPE (EN 10365).
    s(Ipe, 80, 80.0, 46.0, 3.8, 5.2, 6.0),
    s(Ipe, 100, 100.0, 55.0, 4.1, 5.7, 8.1),
    s(Ipe, 120, 120.0, 64.0, 4.4, 6.3, 10.4),
    s(Ipe, 140, 140.0, 73.0, 4.7, 6.9, 12.9),
    s(Ipe, 160, 160.0, 82.0, 5.0, 7.4, 15.8),
    s(Ipe, 180, 180.0, 91.0, 5.3, 8.0, 18.8),
    s(Ipe, 200, 200.0, 100.0, 5.6, 8.5, 22.4),
    s(Ipe, 220, 220.0, 110.0, 5.9, 9.2, 26.2),
    s(Ipe, 240, 240.0, 120.0, 6.2, 9.8, 30.7),
    s(Ipe, 270, 270.0, 135.0, 6.6, 10.2, 36.1),
    s(Ipe, 300, 300.0, 150.0, 7.1, 10.7, 42.2),
    s(Ipe, 330, 330.0, 160.0, 7.5, 11.5, 49.1),
    s(Ipe, 360, 360.0, 170.0, 8.0, 12.7, 57.1),
    s(Ipe, 400, 400.0, 180.0, 8.6, 13.5, 66.3),
    s(Ipe, 450, 450.0, 190.0, 9.4, 14.6, 77.6),
    s(Ipe, 500, 500.0, 200.0, 10.2, 16.0, 90.7),
    s(Ipe, 550, 550.0, 210.0, 11.1, 17.2, 106.0),
    s(Ipe, 600, 600.0, 220.0, 12.0, 19.0, 122.0),
    // HEA (EN 10365): the nominal height is the number, the real one is smaller.
    s(Hea, 100, 96.0, 100.0, 5.0, 8.0, 16.7),
    s(Hea, 120, 114.0, 120.0, 5.0, 8.0, 19.9),
    s(Hea, 140, 133.0, 140.0, 5.5, 8.5, 24.7),
    s(Hea, 160, 152.0, 160.0, 6.0, 9.0, 30.4),
    s(Hea, 180, 171.0, 180.0, 6.0, 9.5, 35.5),
    s(Hea, 200, 190.0, 200.0, 6.5, 10.0, 42.3),
    s(Hea, 220, 210.0, 220.0, 7.0, 11.0, 50.5),
    s(Hea, 240, 230.0, 240.0, 7.5, 12.0, 60.3),
    s(Hea, 260, 250.0, 260.0, 7.5, 12.5, 68.2),
    s(Hea, 280, 270.0, 280.0, 8.0, 13.0, 76.4),
    s(Hea, 300, 290.0, 300.0, 8.5, 14.0, 88.3),
    s(Hea, 320, 310.0, 300.0, 9.0, 15.5, 97.6),
    s(Hea, 340, 330.0, 300.0, 9.5, 16.5, 105.0),
    s(Hea, 360, 350.0, 300.0, 10.0, 17.5, 112.0),
    s(Hea, 400, 390.0, 300.0, 11.0, 19.0, 125.0),
    s(Hea, 450, 440.0, 300.0, 11.5, 21.0, 140.0),
    s(Hea, 500, 490.0, 300.0, 12.0, 23.0, 155.0),
    // HEB (EN 10365).
    s(Heb, 100, 100.0, 100.0, 6.0, 10.0, 20.4),
    s(Heb, 120, 120.0, 120.0, 6.5, 11.0, 26.7),
    s(Heb, 140, 140.0, 140.0, 7.0, 12.0, 33.7),
    s(Heb, 160, 160.0, 160.0, 8.0, 13.0, 42.6),
    s(Heb, 180, 180.0, 180.0, 8.5, 14.0, 51.2),
    s(Heb, 200, 200.0, 200.0, 9.0, 15.0, 61.3),
    s(Heb, 220, 220.0, 220.0, 9.5, 16.0, 71.5),
    s(Heb, 240, 240.0, 240.0, 10.0, 17.0, 83.2),
    s(Heb, 260, 260.0, 260.0, 10.0, 17.5, 93.0),
    s(Heb, 280, 280.0, 280.0, 10.5, 18.0, 103.0),
    s(Heb, 300, 300.0, 300.0, 11.0, 19.0, 117.0),
    s(Heb, 320, 320.0, 300.0, 11.5, 20.5, 127.0),
    s(Heb, 340, 340.0, 300.0, 12.0, 21.5, 134.0),
    s(Heb, 360, 360.0, 300.0, 12.5, 22.5, 142.0),
    s(Heb, 400, 400.0, 300.0, 13.5, 24.0, 155.0),
    s(Heb, 450, 450.0, 300.0, 14.0, 26.0, 171.0),
    s(Heb, 500, 500.0, 300.0, 14.5, 28.0, 187.0),
    // UPN (DIN 1026-1).
    s(Upn, 80, 80.0, 45.0, 6.0, 8.0, 8.64),
    s(Upn, 100, 100.0, 50.0, 6.0, 8.5, 10.6),
    s(Upn, 120, 120.0, 55.0, 7.0, 9.0, 13.4),
    s(Upn, 140, 140.0, 60.0, 7.0, 10.0, 16.0),
    s(Upn, 160, 160.0, 65.0, 7.5, 10.5, 18.8),
    s(Upn, 180, 180.0, 70.0, 8.0, 11.0, 22.0),
    s(Upn, 200, 200.0, 75.0, 8.5, 11.5, 25.3),
    s(Upn, 220, 220.0, 80.0, 9.0, 12.5, 29.4),
    s(Upn, 240, 240.0, 85.0, 9.5, 13.0, 33.2),
    s(Upn, 260, 260.0, 90.0, 10.0, 14.0, 37.9),
    s(Upn, 280, 280.0, 95.0, 10.0, 15.0, 41.8),
    s(Upn, 300, 300.0, 100.0, 10.0, 16.0, 46.2),
];

/// The section `family` `number`, if the table has it.
pub fn find_section(family: SectionFamily, number: u32) -> Option<&'static SteelSection> {
    STEEL_SECTIONS.iter().find(|s| s.family == family && s.number == number)
}

/// The section a designation names ("IPN 200", "hea300", "IPE-240").
pub fn parse_section(text: &str) -> Option<&'static SteelSection> {
    let family = SectionFamily::parse(text)?;
    let digits: String = text.trim().chars().skip_while(|c| c.is_ascii_alphabetic()).filter(char::is_ascii_digit).collect();
    find_section(family, digits.parse().ok()?)
}

/// The sections of a family, from the smallest.
pub fn sections_of(family: SectionFamily) -> impl Iterator<Item = &'static SteelSection> {
    STEEL_SECTIONS.iter().filter(move |s| s.family == family)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mass per metre follows from the shape (7850 kg/m³) within what the fillets and the taper add: a table entry
    /// that is far off is a typing error.
    #[test]
    fn every_mass_agrees_with_the_shape() {
        for s in STEEL_SECTIONS {
            let by_shape = s.shape_area_mm2() * 1e-6 * 7850.0;
            let ratio = s.kg_m / by_shape;
            assert!((0.95..=1.40).contains(&ratio), "{}: {} kg/m against {by_shape:.1} from the shape", s.designation(), s.kg_m);
        }
    }

    #[test]
    fn the_table_is_ordered_unique_and_usable() {
        for f in SectionFamily::ALL {
            let v: Vec<_> = sections_of(f).collect();
            assert!(v.len() >= 12, "{}", f.name());
            assert!(v.windows(2).all(|w| w[0].number < w[1].number && w[0].h_mm <= w[1].h_mm && w[0].kg_m < w[1].kg_m), "{}", f.name());
        }
        for s in STEEL_SECTIONS {
            assert!(s.h_mm > 0.0 && s.b_mm > 0.0 && s.tw_mm > 0.0 && s.tf_mm > 0.0 && s.kg_m > 0.0, "{}", s.designation());
            assert!(2.0 * s.tf_mm < s.h_mm && s.tw_mm < s.b_mm, "{}", s.designation());
        }
    }

    #[test]
    fn designations_are_read_in_the_usual_ways() {
        for t in ["IPN 200", "ipn200", "IPN-200", " Ipn  200 "] {
            assert_eq!(parse_section(t).map(|s| (s.family, s.number)), Some((SectionFamily::Ipn, 200)), "{t}");
        }
        assert_eq!(parse_section("HEA 300").map(|s| s.kg_m), Some(88.3));
        assert_eq!(parse_section("IPN 205"), None);
        assert_eq!(parse_section("XYZ 200"), None);
        assert_eq!(parse_section("200"), None);
        assert_eq!(parse_section(""), None);
        // Known values of the standards: an IPN 200 is 26.2 kg/m and 90 mm wide, an IPE 200 22.4 kg/m, a HEB 200 61.3 kg/m.
        assert_eq!(find_section(SectionFamily::Ipn, 200).map(|s| (s.b_mm, s.kg_m)), Some((90.0, 26.2)));
        assert_eq!(find_section(SectionFamily::Ipe, 200).map(|s| s.kg_m), Some(22.4));
        assert_eq!(find_section(SectionFamily::Heb, 200).map(|s| s.kg_m), Some(61.3));
    }
}
