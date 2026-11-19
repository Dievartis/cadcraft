//! Sanitary fixtures (plumbing): water closets, urinals, basins, sinks, showers, baths, bidets, slop sinks, floor drains, washing machines,
//! dishwashers, water heaters and draw-off taps. Each is a block reference drawn in plan at its size, seen from above, with its parameters in
//! extended data: where it drains (waste water or soil water) and how wide its drain is, the base flows of cold and hot water it draws, and the
//! altitude of its underside. Its drain outlet is where "Connect Fixtures" starts the branch to the nearest drain of its kind.
//!
//! The sizes, drain diameters and base flows are the usual values in France (DTU 60.11), given as defaults the user changes.
//! The symbols are drawn here, in code: the outline of the fixture and the few lines that tell one from another.

use cadcraft_geom::{PolyVertex, Vec2};
use serde::{Deserialize, Serialize};

use super::{HvacPiece, HvacUnits, PipeService, check_altitude, check_label_height, invalid, label, v3, xdata};
use crate::{Circle, Ellipse, EntityKind, Line, LwPolyline, RawTag, Result};

pub const SANITARY_APP: &str = "CADCRAFT_SANITARY";
const SANITARY_FORMAT: &str = "SANITARY1";
/// The layer of the fixtures.
pub const SANITARY_LAYER: &str = "P-SANR-FIXT";
/// The widest or deepest fixture, mm.
const MAX_SIZE_MM: f64 = 5000.0;

/// A kind of sanitary fixture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SanitaryKind {
    /// A wall-hung water closet (WC suspendu).
    #[default]
    WcWallHung,
    /// A floor-standing water closet with its cistern (WC au sol).
    WcFloor,
    /// An accessible water closet, with its grab bar (WC PMR).
    WcAccessible,
    Urinal,
    /// A washbasin (lavabo).
    Washbasin,
    /// A hand basin (lave-mains).
    HandBasin,
    /// A vanity top with two basins (plan double vasque).
    DoubleBasin,
    /// A kitchen sink with one bowl and a drainer (évier 1 bac).
    SinkSingle,
    /// A kitchen sink with two bowls and a drainer (évier 2 bacs).
    SinkDouble,
    /// A shower tray (receveur de douche).
    Shower,
    /// A bath (baignoire).
    Bath,
    Bidet,
    /// A slop sink (vidoir).
    SlopSink,
    /// A floor drain (siphon de sol).
    FloorDrain,
    /// A washing machine (lave-linge).
    WashingMachine,
    /// A dishwasher (lave-vaisselle).
    Dishwasher,
    /// An electric water heater (chauffe-eau, ballon d'eau chaude).
    WaterHeater,
    /// A draw-off tap (robinet de puisage).
    Tap,
}

impl SanitaryKind {
    pub const ALL: [SanitaryKind; 18] = [
        SanitaryKind::WcWallHung,
        SanitaryKind::WcFloor,
        SanitaryKind::WcAccessible,
        SanitaryKind::Urinal,
        SanitaryKind::Washbasin,
        SanitaryKind::HandBasin,
        SanitaryKind::DoubleBasin,
        SanitaryKind::SinkSingle,
        SanitaryKind::SinkDouble,
        SanitaryKind::Shower,
        SanitaryKind::Bath,
        SanitaryKind::Bidet,
        SanitaryKind::SlopSink,
        SanitaryKind::FloorDrain,
        SanitaryKind::WashingMachine,
        SanitaryKind::Dishwasher,
        SanitaryKind::WaterHeater,
        SanitaryKind::Tap,
    ];

    /// The name a command takes.
    pub fn name(self) -> &'static str {
        match self {
            SanitaryKind::WcWallHung => "wcWallHung",
            SanitaryKind::WcFloor => "wcFloor",
            SanitaryKind::WcAccessible => "wcAccessible",
            SanitaryKind::Urinal => "urinal",
            SanitaryKind::Washbasin => "washbasin",
            SanitaryKind::HandBasin => "handBasin",
            SanitaryKind::DoubleBasin => "doubleBasin",
            SanitaryKind::SinkSingle => "sinkSingle",
            SanitaryKind::SinkDouble => "sinkDouble",
            SanitaryKind::Shower => "shower",
            SanitaryKind::Bath => "bath",
            SanitaryKind::Bidet => "bidet",
            SanitaryKind::SlopSink => "slopSink",
            SanitaryKind::FloorDrain => "floorDrain",
            SanitaryKind::WashingMachine => "washingMachine",
            SanitaryKind::Dishwasher => "dishwasher",
            SanitaryKind::WaterHeater => "waterHeater",
            SanitaryKind::Tap => "tap",
        }
    }

    /// The kind `text` names: its command name, or a common French or English word for it.
    pub fn parse(text: &str) -> Option<SanitaryKind> {
        let t: String = text.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        SanitaryKind::ALL.into_iter().find(|k| k.name().to_lowercase() == t).or(match t.as_str() {
            "wc" | "wcsuspendu" | "toilet" => Some(SanitaryKind::WcWallHung),
            "wcausol" | "wcsol" => Some(SanitaryKind::WcFloor),
            "wcpmr" | "pmr" => Some(SanitaryKind::WcAccessible),
            "urinoir" => Some(SanitaryKind::Urinal),
            "lavabo" | "basin" => Some(SanitaryKind::Washbasin),
            "lavemains" => Some(SanitaryKind::HandBasin),
            "vasque" | "doublevasque" | "plandoublevasque" => Some(SanitaryKind::DoubleBasin),
            "evier" | "évier" | "sink" | "evier1bac" => Some(SanitaryKind::SinkSingle),
            "evier2bacs" => Some(SanitaryKind::SinkDouble),
            "douche" => Some(SanitaryKind::Shower),
            "baignoire" | "bathtub" => Some(SanitaryKind::Bath),
            "vidoir" => Some(SanitaryKind::SlopSink),
            "siphondesol" | "siphon" => Some(SanitaryKind::FloorDrain),
            "lavelinge" => Some(SanitaryKind::WashingMachine),
            "lavevaisselle" => Some(SanitaryKind::Dishwasher),
            "chauffeeau" | "ballon" | "cumulus" => Some(SanitaryKind::WaterHeater),
            "robinet" | "robinetdepuisage" => Some(SanitaryKind::Tap),
            _ => None,
        })
    }

    /// The short code written beside the fixture.
    pub fn code(self) -> &'static str {
        match self {
            SanitaryKind::WcWallHung | SanitaryKind::WcFloor => "WC",
            SanitaryKind::WcAccessible => "WC PMR",
            SanitaryKind::Urinal => "URI",
            SanitaryKind::Washbasin => "LAV",
            SanitaryKind::HandBasin => "LM",
            SanitaryKind::DoubleBasin => "VAS",
            SanitaryKind::SinkSingle | SanitaryKind::SinkDouble => "EVI",
            SanitaryKind::Shower => "DOU",
            SanitaryKind::Bath => "BAI",
            SanitaryKind::Bidet => "BID",
            SanitaryKind::SlopSink => "VID",
            SanitaryKind::FloorDrain => "SIP",
            SanitaryKind::WashingMachine => "LL",
            SanitaryKind::Dishwasher => "LV",
            SanitaryKind::WaterHeater => "CE",
            SanitaryKind::Tap => "RP",
        }
    }

    /// The usual size, mm: the width along the wall it stands against, and the depth into the room.
    pub fn default_size_mm(self) -> (f64, f64) {
        match self {
            SanitaryKind::WcWallHung | SanitaryKind::Bidet => (360.0, 540.0),
            SanitaryKind::WcFloor | SanitaryKind::WcAccessible => (380.0, 700.0),
            SanitaryKind::Urinal => (360.0, 300.0),
            SanitaryKind::Washbasin => (550.0, 450.0),
            SanitaryKind::HandBasin => (400.0, 250.0),
            SanitaryKind::DoubleBasin => (1200.0, 500.0),
            SanitaryKind::SinkSingle => (800.0, 500.0),
            SanitaryKind::SinkDouble => (1200.0, 600.0),
            SanitaryKind::Shower => (900.0, 900.0),
            SanitaryKind::Bath => (1700.0, 700.0),
            SanitaryKind::SlopSink => (500.0, 400.0),
            SanitaryKind::FloorDrain => (150.0, 150.0),
            SanitaryKind::WashingMachine | SanitaryKind::Dishwasher => (600.0, 600.0),
            SanitaryKind::WaterHeater => (550.0, 550.0),
            SanitaryKind::Tap => (80.0, 80.0),
        }
    }

    /// Where it drains: soil water (water closets, urinals, slop sinks) or waste water (the others); `None` for a tap.
    pub fn default_drain(self) -> Option<PipeService> {
        match self {
            SanitaryKind::WcWallHung | SanitaryKind::WcFloor | SanitaryKind::WcAccessible | SanitaryKind::Urinal | SanitaryKind::SlopSink => {
                Some(PipeService::SoilWater)
            }
            SanitaryKind::Tap => None,
            _ => Some(PipeService::WasteWater),
        }
    }

    /// The outer diameter of its drain, mm (PVC).
    pub fn default_drain_dn_mm(self) -> f64 {
        match self {
            SanitaryKind::WcWallHung | SanitaryKind::WcFloor | SanitaryKind::WcAccessible | SanitaryKind::SlopSink => 100.0,
            SanitaryKind::HandBasin | SanitaryKind::Bidet | SanitaryKind::WaterHeater => 32.0,
            SanitaryKind::FloorDrain => 50.0,
            SanitaryKind::Tap => 0.0,
            _ => 40.0,
        }
    }

    /// The base flows of cold and of hot water it draws, l/s.
    pub fn default_flows_ls(self) -> (Option<f64>, Option<f64>) {
        match self {
            SanitaryKind::WcWallHung | SanitaryKind::WcFloor | SanitaryKind::WcAccessible => (Some(0.12), None),
            SanitaryKind::Urinal => (Some(0.15), None),
            SanitaryKind::Washbasin | SanitaryKind::SinkSingle | SanitaryKind::SinkDouble | SanitaryKind::Shower | SanitaryKind::Bidet => {
                (Some(0.2), Some(0.2))
            }
            SanitaryKind::SlopSink => (Some(0.2), Some(0.2)),
            SanitaryKind::HandBasin => (Some(0.1), None),
            SanitaryKind::DoubleBasin => (Some(0.4), Some(0.4)),
            SanitaryKind::Bath => (Some(0.33), Some(0.33)),
            SanitaryKind::WashingMachine => (Some(0.2), None),
            SanitaryKind::Dishwasher => (Some(0.1), None),
            SanitaryKind::Tap => (Some(0.33), None),
            SanitaryKind::FloorDrain | SanitaryKind::WaterHeater => (None, None),
        }
    }

    /// The altitude of its underside (of its rim, for a basin or a sink), mm above the floor.
    pub fn default_bottom_mm(self) -> f64 {
        match self {
            SanitaryKind::WcWallHung => 400.0,
            SanitaryKind::Urinal | SanitaryKind::SlopSink | SanitaryKind::Tap => 600.0,
            SanitaryKind::Washbasin | SanitaryKind::HandBasin | SanitaryKind::DoubleBasin => 850.0,
            SanitaryKind::SinkSingle | SanitaryKind::SinkDouble => 900.0,
            _ => 0.0,
        }
    }

    /// Where its drain is, mm, in its own frame (the wall side along x at y = 0, the room toward -y), at its usual size.
    fn outlet_mm(self) -> (f64, f64) {
        let (w, d) = self.default_size_mm();
        match self {
            SanitaryKind::WcWallHung | SanitaryKind::Urinal | SanitaryKind::WashingMachine | SanitaryKind::Dishwasher => (0.0, -60.0),
            SanitaryKind::WcFloor | SanitaryKind::WcAccessible => (0.0, -260.0),
            SanitaryKind::Washbasin | SanitaryKind::HandBasin | SanitaryKind::SlopSink => (0.0, -100.0),
            SanitaryKind::DoubleBasin => (-w / 4.0, -120.0),
            SanitaryKind::SinkSingle => (-w / 4.0, -d / 2.0),
            SanitaryKind::SinkDouble => (-w / 4.0, -d / 2.0),
            SanitaryKind::Bath => (-w / 2.0 + 250.0, -d / 2.0),
            SanitaryKind::Bidet => (0.0, -120.0),
            _ => (0.0, -d / 2.0),
        }
    }
}

fn default_label_mm() -> f64 {
    100.0
}

/// A sanitary fixture: what its block reference keeps. The block's origin is the middle of the side against the wall; the fixture
/// stands toward the block's -y.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitaryParams {
    pub kind: SanitaryKind,
    /// The tag written beside the fixture; empty for none (the code of its kind is written).
    #[serde(default)]
    pub name: String,
    /// Its width along the wall and its depth into the room, mm.
    pub width_mm: f64,
    pub depth_mm: f64,
    /// Where it drains (waste water or soil water); `None` when it has no drain.
    #[serde(default)]
    pub drain: Option<PipeService>,
    /// The outer diameter of its drain, mm.
    #[serde(default)]
    pub drain_dn_mm: f64,
    /// The base flows of cold and hot water it draws, l/s; `None` when it takes none.
    #[serde(default)]
    pub cold_ls: Option<f64>,
    #[serde(default)]
    pub hot_ls: Option<f64>,
    /// The altitude of its underside, mm above the drawing's zero.
    #[serde(default)]
    pub bottom_mm: Option<f64>,
    #[serde(default = "default_label_mm")]
    pub label_height_mm: f64,
    #[serde(default)]
    pub units: HvacUnits,
}

impl SanitaryParams {
    /// A fixture of `kind` at its usual size and with its usual values.
    pub fn new(kind: SanitaryKind, units: HvacUnits) -> SanitaryParams {
        let (width_mm, depth_mm) = kind.default_size_mm();
        let (cold_ls, hot_ls) = kind.default_flows_ls();
        SanitaryParams {
            kind,
            name: String::new(),
            width_mm,
            depth_mm,
            drain: kind.default_drain(),
            drain_dn_mm: kind.default_drain_dn_mm(),
            cold_ls,
            hot_ls,
            bottom_mm: Some(kind.default_bottom_mm()),
            label_height_mm: default_label_mm(),
            units,
        }
    }

    pub fn validate(&self) -> Result<()> {
        check_label_height(self.label_height_mm)?;
        self.units.validate()?;
        check_altitude(self.bottom_mm)?;
        let size = |v: f64| v.is_finite() && (20.0..=MAX_SIZE_MM).contains(&v);
        if !size(self.width_mm) || !size(self.depth_mm) {
            return Err(invalid(format!("A fixture's width and depth are 20 to {MAX_SIZE_MM} mm.")));
        }
        if self.drain.is_some_and(|d| !d.is_gravity_drain()) {
            return Err(invalid("A fixture drains to waste water, soil water or rain water."));
        }
        if !(self.drain_dn_mm.is_finite() && (0.0..=400.0).contains(&self.drain_dn_mm)) || (self.drain.is_some() && self.drain_dn_mm < 20.0) {
            return Err(invalid("The drain of a fixture is 20 to 400 mm across."));
        }
        if [self.cold_ls, self.hot_ls].iter().flatten().any(|q| !(q.is_finite() && (0.0..=20.0).contains(q))) {
            return Err(invalid("A fixture's flows are 0 to 20 l/s."));
        }
        if self.name.chars().count() > 40 {
            return Err(invalid("A fixture's name is 40 characters at most."));
        }
        Ok(())
    }

    /// Its scale in x and in y against the usual size of its kind.
    fn stretch(&self) -> (f64, f64) {
        let (w, d) = self.kind.default_size_mm();
        (self.width_mm / w, self.depth_mm / d)
    }

    /// Where its drain is, in the block's coordinates (drawing units); `None` when it has no drain.
    pub fn outlet(&self) -> Option<Vec2> {
        self.drain?;
        let (x, y) = self.kind.outlet_mm();
        let (sx, sy) = self.stretch();
        Some(Vec2::new(self.units.to_units(x * sx), self.units.to_units(y * sy)))
    }

    /// The text written beside it: its name or the code of its kind, then its drain ("EU Ø40").
    pub fn label_text(&self) -> String {
        let mut t = if self.name.trim().is_empty() { self.kind.code().to_string() } else { self.name.trim().to_string() };
        if let Some(d) = self.drain {
            t.push_str(&format!(" {} \u{d8}{:.0}", d.tag_code(), self.drain_dn_mm));
        }
        t
    }

    /// The fixture seen from above at its size, and its text below it.
    pub fn generate(&self) -> Result<Vec<HvacPiece>> {
        self.generate_turned(false)
    }

    /// [`generate`](Self::generate), the text turned half a turn when `flip_label`: a fixture turned so that its text would read upside down.
    pub fn generate_turned(&self, flip_label: bool) -> Result<Vec<HvacPiece>> {
        self.validate()?;
        let (sx, sy) = self.stretch();
        let u = |mm: f64| self.units.to_units(mm);
        let p = |x: f64, y: f64| Vec2::new(u(x * sx), u(y * sy));
        let mut shapes: Vec<EntityKind> = Vec::new();
        let (w, d) = self.kind.default_size_mm();
        let (hw, hd) = (w / 2.0, d);
        let rect = |x0: f64, y0: f64, x1: f64, y1: f64| poly(&[p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)], true);
        let line = |a: (f64, f64), b: (f64, f64)| EntityKind::Line(Line { a: v3(p(a.0, a.1)), b: v3(p(b.0, b.1)) });
        let ellipse = |cx: f64, cy: f64, rx: f64, ry: f64| ellipse(p(cx, cy), u(rx * sx), u(ry * sy));
        let circle = |cx: f64, cy: f64, r: f64| EntityKind::Circle(Circle { center: v3(p(cx, cy)), radius: u(r * sx.min(sy)) });
        match self.kind {
            SanitaryKind::WcWallHung => {
                shapes.push(rect(-hw, 0.0, hw, -80.0));
                shapes.push(ellipse(0.0, -300.0, 160.0, 225.0));
                shapes.push(ellipse(0.0, -310.0, 105.0, 160.0));
            }
            SanitaryKind::WcFloor | SanitaryKind::WcAccessible => {
                shapes.push(rect(-hw, 0.0, hw, -180.0));
                shapes.push(ellipse(0.0, -450.0, 170.0, 250.0));
                shapes.push(ellipse(0.0, -460.0, 110.0, 170.0));
                if self.kind == SanitaryKind::WcAccessible {
                    // The grab bar beside it.
                    shapes.push(line((hw + 300.0, -150.0), (hw + 300.0, -750.0)));
                }
            }
            SanitaryKind::Urinal => {
                shapes.push(EntityKind::LwPolyline(LwPolyline {
                    vertices: vec![vertex(p(-hw, 0.0), 0.0), vertex(p(hw, 0.0), 0.0), vertex(p(hw, -120.0), 1.0), vertex(p(-hw, -120.0), 0.0)],
                    closed: true,
                    const_width: 0.0,
                    elevation: 0.0,
                    plinegen: false,
                }));
                shapes.push(circle(0.0, -60.0, 20.0));
            }
            SanitaryKind::Washbasin | SanitaryKind::HandBasin => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                shapes.push(ellipse(0.0, -hd * 0.55, hw * 0.7, hd * 0.3));
                shapes.push(circle(0.0, -hd * 0.13, 15.0));
            }
            SanitaryKind::DoubleBasin => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                for x in [-hw / 2.0, hw / 2.0] {
                    shapes.push(ellipse(x, -hd * 0.55, 180.0, 130.0));
                    shapes.push(circle(x, -70.0, 15.0));
                }
            }
            SanitaryKind::SinkSingle | SanitaryKind::SinkDouble => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                let bowls: &[(f64, f64)] = if self.kind == SanitaryKind::SinkSingle {
                    &[(-hw + 50.0, -10.0)]
                } else {
                    &[(-hw + 50.0, -hw / 3.0), (-hw / 3.0 + 20.0, 120.0)]
                };
                for (x0, x1) in bowls {
                    shapes.push(rect(*x0, -80.0, *x1, -hd + 60.0));
                }
                // The drainer's grooves.
                let from = bowls.last().map_or(0.0, |b| b.1) + 60.0;
                for k in 1..=4 {
                    let y = -hd * f64::from(k) / 5.0;
                    shapes.push(line((from, y), (hw - 40.0, y)));
                }
                shapes.push(circle(bowls.first().map_or(0.0, |b| (b.0 + b.1) / 2.0), -hd / 2.0, 25.0));
            }
            SanitaryKind::Shower => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                shapes.push(line((-hw, 0.0), (hw, -hd)));
                shapes.push(line((hw, 0.0), (-hw, -hd)));
                shapes.push(circle(0.0, -hd / 2.0, 50.0));
            }
            SanitaryKind::Bath => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                shapes.push(rect(-hw + 80.0, -80.0, hw - 80.0, -hd + 80.0));
                shapes.push(circle(-hw + 250.0, -hd / 2.0, 30.0));
            }
            SanitaryKind::Bidet => {
                shapes.push(ellipse(0.0, -hd / 2.0, hw, hd / 2.0));
                shapes.push(ellipse(0.0, -hd * 0.55, hw * 0.55, hd * 0.32));
                shapes.push(circle(0.0, -60.0, 15.0));
            }
            SanitaryKind::SlopSink => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                shapes.push(rect(-hw + 60.0, -100.0, hw - 60.0, -hd + 40.0));
            }
            SanitaryKind::FloorDrain => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                for k in 1..=3 {
                    let y = -hd * f64::from(k) / 4.0;
                    shapes.push(line((-hw + 20.0, y), (hw - 20.0, y)));
                }
            }
            SanitaryKind::WashingMachine => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                shapes.push(circle(0.0, -hd / 2.0 - 20.0, 190.0));
                shapes.push(circle(0.0, -hd / 2.0 - 20.0, 130.0));
            }
            SanitaryKind::Dishwasher => {
                shapes.push(rect(-hw, 0.0, hw, -hd));
                for k in 1..=5 {
                    let x = -hw + w * f64::from(k) / 6.0;
                    shapes.push(line((x, -100.0), (x, -hd + 60.0)));
                }
            }
            SanitaryKind::WaterHeater => {
                shapes.push(circle(0.0, -hd / 2.0, hw));
                shapes.push(circle(0.0, -hd / 2.0, hw - 50.0));
            }
            SanitaryKind::Tap => {
                shapes.push(circle(0.0, -hd / 2.0, hw));
                shapes.push(line((0.0, 0.0), (0.0, -hd / 2.0 + hw)));
            }
        }
        let mut pieces: Vec<HvacPiece> = shapes.into_iter().map(|kind| HvacPiece { layer: SANITARY_LAYER, kind }).collect();
        let h = u(self.label_height_mm);
        let below = -u(self.depth_mm.max(if self.kind == SanitaryKind::WcAccessible { 750.0 * sy } else { 0.0 })) - 1.3 * h;
        let mut text = label(Vec2::new(0.0, below), h, self.label_text());
        if let (EntityKind::Text(t), true) = (&mut text, flip_label) {
            t.rotation = std::f64::consts::PI;
        }
        pieces.push(HvacPiece { layer: SANITARY_LAYER, kind: text });
        Ok(pieces)
    }

    pub fn to_xdata(&self) -> Result<Vec<RawTag>> {
        xdata::to_xdata(SANITARY_APP, SANITARY_FORMAT, self)
    }

    pub fn from_xdata(tags: &[RawTag]) -> Option<SanitaryParams> {
        let p: SanitaryParams = xdata::from_xdata(SANITARY_APP, SANITARY_FORMAT, tags)?;
        p.validate().is_ok().then_some(p)
    }
}

fn vertex(p: Vec2, bulge: f64) -> PolyVertex {
    PolyVertex { p, bulge, start_width: 0.0, end_width: 0.0 }
}

fn poly(points: &[Vec2], closed: bool) -> EntityKind {
    EntityKind::LwPolyline(LwPolyline {
        vertices: points.iter().map(|p| vertex(*p, 0.0)).collect(),
        closed,
        const_width: 0.0,
        elevation: 0.0,
        plinegen: false,
    })
}

/// A whole ellipse of half axes `rx` (along x) and `ry`, the longer one as its major axis.
fn ellipse(c: Vec2, rx: f64, ry: f64) -> EntityKind {
    let (major, ratio) = if rx >= ry { (Vec2::new(rx, 0.0), ry / rx.max(1e-12)) } else { (Vec2::new(0.0, ry), rx / ry.max(1e-12)) };
    EntityKind::Ellipse(Ellipse { center: v3(c), major: v3(major), ratio: ratio.clamp(1e-6, 1.0), start: 0.0, end: std::f64::consts::TAU })
}

/// The fixture an object is, when it is a block reference carrying usable fixture data.
pub fn sanitary_of(e: &crate::Entity) -> Option<SanitaryParams> {
    match &e.kind {
        EntityKind::Insert(_) => SanitaryParams::from_xdata(&e.common.xdata),
        _ => None,
    }
}

/// The colour the fixtures' layer is created with.
pub fn sanitary_layer_color(name: &str) -> Option<cadcraft_color::Color> {
    (name == SANITARY_LAYER).then_some(cadcraft_color::Color::True(cadcraft_color::Rgb(0xd8, 0xdd, 0xe4)))
}

impl crate::Drawing {
    /// Adds a fixture of `p` at `at` (the middle of its side against the wall), turned `rotation` radians.
    pub fn add_sanitary(
        &mut self,
        space: &crate::Space,
        mut common: crate::Common,
        p: &SanitaryParams,
        at: Vec2,
        rotation: f64,
    ) -> Result<crate::Handle> {
        if !at.is_finite() || !rotation.is_finite() {
            return Err(invalid("A fixture needs a usable position."));
        }
        let pieces = p.generate_turned(super::duct::reads_upside_down(rotation))?;
        common.xdata = xdata::replace(SANITARY_APP, &common.xdata, p.to_xdata()?);
        common.layer = SANITARY_LAYER.into();
        let block = self.hvac_block(pieces);
        self.ensure_layer(SANITARY_LAYER);
        let insert = crate::Insert { block, insert: v3(at), rotation, ..Default::default() };
        self.add(space, common, EntityKind::Insert(insert))
    }

    /// Gives fixture `h` the parameters `p`, redrawing it in place.
    pub fn regenerate_sanitary(&mut self, h: crate::Handle, p: &SanitaryParams) -> Result<()> {
        let e = self.entity(h).ok_or(crate::DocError::NoEntity(h))?;
        let turned = matches!(&e.kind, EntityKind::Insert(i) if super::duct::reads_upside_down(i.rotation));
        let pieces = p.generate_turned(turned)?;
        let tags = xdata::replace(SANITARY_APP, &e.common.xdata, p.to_xdata()?);
        self.regenerate_hvac(h, pieces, tags)
    }

    /// Where the drain of fixture `h` is, in drawing coordinates, with the fixture; `None` when `h` is no fixture or has no drain.
    pub fn sanitary_outlet(&self, h: crate::Handle) -> Option<(SanitaryParams, Vec2)> {
        let e = self.entity(h)?;
        let EntityKind::Insert(i) = &e.kind else { return None };
        let p = sanitary_of(e)?;
        let local = p.outlet()?;
        Some((p, super::slab::block_to_world(i, local)))
    }
}
