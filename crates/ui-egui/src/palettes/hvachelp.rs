//! The "i" beside an HVAC tool: a window with a drawing of what the tool does (seen from above, as
//! on the plan) and the steps to follow. The drawings are made here, from lines and arcs.

use egui::{Color32, Painter, Rect, Stroke, pos2, vec2};

use crate::theme::Tokens;

/// One tool's help: its English name (the tool's label), the steps, and its drawing.
struct Help {
    id: &'static str,
    steps: &'static [&'static str],
    figure: fn(&Painter, Rect),
}

const OPEN: &str = "hvac_help_open";

const HELPS: &[Help] = &[
    Help {
        id: "Points List",
        steps: &[
            "Place the controllers and the modules and give each a name: the devices are wired to them by that name.",
            "Place the sensors, the thermostats and the actuators; in Properties type the controller each is wired to and the equipment it serves.",
            "Check Controls tells what is wrong: a device wired to nothing, a controller with more points than it offers, no circuit that feeds it, a bus with no cable.",
            "The Points List shows the load and the reserve of each controller and writes the workbook: one line for each point, with the room it is in.",
        ],
        figure: points_list,
    },
    Help {
        id: "Distribute in a Space",
        steps: &[
            "Start the tool (air terminals) or Distribute Units in a Space (indoor units), then select the space.",
            "Type how many: 6 spreads six in the most even grid the room's shape allows; 2x3 makes 2 rows by 3 columns.",
            "A grid cell whose centre is outside the space gets nothing, so an L-shaped room is filled by its two arms.",
            "Terminals share the space's flow; units get their model's power. Edit one afterwards, or move it by its grip.",
        ],
        figure: distribute,
    },
    Help {
        id: "Riser",
        steps: &[
            "Start the tool and click the corners of the shaft, then press Enter.",
            "Type the flow that comes from the other floors in Properties.",
            "Size Ducts gives the riser its section; the arrow shows if the air goes up or down.",
        ],
        figure: riser,
    },
    Help {
        id: "Duct",
        steps: &[
            "Click the points of the run: one duct is drawn per segment.",
            "Start on a unit's collar, end on a terminal; start on another duct to make a tee.",
            "Leave the direction on auto: Size Ducts finds supply or extract from the terminals the run reaches.",
        ],
        figure: duct,
    },
    Help {
        id: "Height Zone",
        steps: &[
            "Click the corners of the outline where ducts have less height (a beam, a low ceiling), seen from above.",
            "Type the height available for the ducts, in mm. An altitude is optional.",
            "Size Ducts cuts the ducts at the edges and sizes them flat inside the zone.",
        ],
        figure: zone,
    },
    Help {
        id: "Connect Terminals",
        steps: &[
            "No selection needed: every terminal that no duct reaches is taken.",
            "A flexible duct is drawn from each one to the nearest duct, 3 m at most.",
            "Terminals too far from any duct are listed and left alone.",
        ],
        figure: connect,
    },
    Help {
        id: "Size Ducts",
        steps: &[
            "Run it when the ducts and terminals are drawn; it reads each network from its unit or riser.",
            "Each duct gets its section, and its flow and velocity are written on it.",
            "Run it again after any change: sections and flows are recalculated.",
        ],
        figure: size,
    },
    Help {
        id: "Duct Crossing",
        steps: &[
            "Click the two ducts that cross.",
            "Choose which one passes under or over the other.",
            "It is drawn broken, or with four 45° elbows when the duct has to go round the other.",
        ],
        figure: crossing,
    },
    Help {
        id: "HVAC Quantities",
        steps: &[
            "Draw one drawing per level, then open this tool on the level.",
            "The tables count ducts by section, fittings, terminals, units, risers and equipment.",
            "The spreadsheet button saves them for Excel; each level makes its own file.",
        ],
        figure: quantities,
    },
    Help {
        id: "Piping",
        steps: &[
            "Choose a service (chilled water, hot water, change-over, condensate, refrigerant), then click the points of the run: one pipe is drawn per segment, seen from above.",
            "At the command line: Service, Leg (supply or return), Diameter (inches for refrigerant: 3/8, 1 1/8) and Insulation. A refrigerant link draws its liquid and gas tubes as one line.",
            "Each service has its own colour. To size a network, mark the run that starts at the source, type the loads at the free ends, then use Size Pipes.",
            "A pipe is joined to another only where their ends meet: start a branch at the end of a run.",
        ],
        figure: piping,
    },
    Help {
        id: "Size Pipes",
        steps: &[
            "It sizes chilled water, hot water, change-over and condensate. Refrigerant links are not sized: type their diameters in inches.",
            "Each network needs a source: tick \"Starts at the source\" on its first run, or start the run on a chiller or in the shaft of a pipe riser.",
            "Type the load at each free end (kW, l/h for a drain), or end the run in a unit or an air handling unit that has a power: its coil counts.",
            "Run it: each pipe gets its size, flow and velocity, and the pressure lost on the most unfavourable circuit is told. A size you typed is checked, not changed. Run it again after any change.",
        ],
        figure: size_pipes,
    },
    Help {
        id: "Pipe Riser",
        steps: &[
            "Click the corners of the shaft, then press Enter. Options: Name (the same on every floor), Lines, Height and Up or Down.",
            "In Lines, type what goes through the shaft, separated by commas: CHW-S, CHW-R, HW-S, HW-R, CO-S, CO-R, COND, REF (liquid and gas), REF-HR (three tubes), REF-L, REF-G.",
            "In Properties: the size of each line (inches for refrigerant) and what it carries to the other floors (kW, l/h for a drain, capacity index for refrigerant). Add or remove lines there, and type the height from floor to floor.",
            "End the pipes of the floor in the shaft, or start them there: they are read through it. Size Pipes sizes the lines, and the takeoff counts them by the height.",
        ],
        figure: pipe_riser,
    },
    Help {
        id: "Name Networks",
        steps: &[
            "A network is the pipes of one service that are joined end to end, through the parts on them, the units they end in and the shaft of a riser.",
            "This tool names every network that has no name (CHW1, HW1…) and gives each its own colour, so that two networks of the same service can be told apart.",
            "Type a name on one pipe in Properties to name its whole network; there you also choose its colour and, for a print without colour, its line type.",
            "A pipe or a part drawn on a named network takes its name. The legend and the takeoff follow the names.",
        ],
        figure: name_networks,
    },
    Help {
        id: "Network Legend",
        steps: &[
            "Click where the upper left corner of the legend goes.",
            "It has a row for each network: a sample in its colour and line type, its name, its pipes and their length.",
            "It is redrawn by itself when a network is named, renamed or restyled.",
        ],
        figure: network_legend,
    },
    Help {
        id: "Library…",
        steps: &[
            "Click a part, then click where it goes; or drag it onto the drawing to drop it there.",
            "A valve, a filter or a pump clicked on a pipe turns along it and cuts it. For the others you are asked the orientation: a second click, an angle, or Enter.",
            "In Properties: its tag, its size (height, width, depth), volume, power, pressure loss, the outlets of a box or a header, and another model of the same kind.",
            "Keep a part of the drawing as a model of your own (My models): it is saved with the drawing.",
        ],
        figure: library,
    },
    Help {
        id: "Check Refrigerant",
        steps: &[
            "Draw the outdoor unit, the links (one line for the liquid and the gas), the Y joints, the headers and the branch boxes, and the indoor units. The pipes must end on them.",
            "It follows the network from each outdoor unit and counts the units and the capacity index (cooling kW × 10) at each outlet.",
            "It tells what is over the limits: 5 units and index 140 per outlet of a box, and the joint or header that suits the index below it.",
            "A network can start or end at a pipe riser: what the other floors serve is typed on its line.",
        ],
        figure: check_refrigerant,
    },
    Help {
        id: "Convert to Openings",
        steps: &[
            "It looks at the door and window blocks of a plan that lie on walls (by the words in their block or layer names).",
            "It lists what it found; accept, and each becomes an opening you can change in Properties.",
            "The old blocks are kept on a frozen layer, A-OPENING-OLD: thaw it to see them.",
        ],
        figure: convert_openings,
    },
    Help {
        id: "Wall Elevation",
        steps: &[
            "Click the wall, then the point where the elevation is drawn, next to the plan.",
            "It shows the wall seen from the side, with its openings, sills and heights.",
            "Run it again after changing the wall or its openings.",
        ],
        figure: wall_elevation,
    },
    Help {
        id: "Wall Junctions",
        steps: &[
            "Walls are joined where they meet (corner, tee or cross) as they are drawn, by the priority of their layers.",
            "After moving, copying or erasing walls, run this tool: it joins them again.",
            "A wall drawn first gives way to a wall of higher priority; layers of equal priority are joined without a line.",
        ],
        figure: wall_junctions,
    },
    Help {
        id: "Corner Window",
        steps: &[
            "Click near the corner of two walls that meet.",
            "A window is put on each wall, from the corner, of the length given by the Width option (1200 mm by default).",
            "They are two ordinary openings: change each of them in Properties.",
        ],
        figure: corner_window,
    },
    Help {
        id: "Ceiling Grid",
        steps: &[
            "Click the corners of the room, then press Enter. Or choose Walls and click inside a room closed by walls, or Room or Object.",
            "In Properties type the tile (600x600, 600x1200…) and the height of the false ceiling.",
            "The whole and cut tiles are counted in the Structure Takeoff; the tile centres are where a diffuser or a luminaire snaps.",
        ],
        figure: ceiling_grid,
    },
    Help {
        id: "Stair",
        steps: &[
            "Click the middle of the first riser, then a point in the direction of the way up.",
            "In Properties type the rise from floor to floor, the largest riser, the tread and the width: the number of risers follows.",
            "The label says whether the stair is comfortable: twice the riser plus the tread within 600 to 650 mm (Blondel).",
        ],
        figure: stair,
    },
    Help {
        id: "Railing",
        steps: &[
            "Click the points of the path of the railing, then press Enter.",
            "In Properties type the height and the spacing of the posts.",
            "The railings and their length are counted in the Structure Takeoff.",
        ],
        figure: railing,
    },
    Help {
        id: "Curtain Wall",
        steps: &[
            "Click the points of the façade, then press Enter. The Height option sets its height.",
            "In Properties type the spacing of the mullions and of the transoms, and the depth of the frame.",
            "The panels, the mullions, the transoms and the area are counted in the Structure Takeoff.",
        ],
        figure: curtain_wall,
    },
    Help {
        id: "Building Section",
        steps: &[
            "Click the two ends of the cut line across the plan, so that it goes through the walls, slabs, columns and beams to show.",
            "Click where the section goes, or press Enter to put it to the right of the drawing.",
            "The cut walls are drawn with their openings, with the slabs, steel columns and beams and false ceilings at their elevations, and the levels in the margin.",
            "It is a picture made now: draw a new section after changing the plan. Slopes are not drawn.",
        ],
        figure: building_section,
    },
    Help {
        id: "Roof",
        steps: &[
            "Click the corners of the roof, then press Enter. Or choose Walls and click inside a room closed by walls.",
            "Choose Shed, Gable or Hip and the pitch in degrees with the options; the ridge runs along the first side.",
            "The label gives the top of the roof. In Properties change the kind, the pitch, the direction of the ridge and the height of the eaves.",
            "The area in plan, the area of the slopes, the rise and the ridge are counted in the Structure Takeoff.",
        ],
        figure: roof,
    },
    Help {
        id: "Column",
        steps: &[
            "Choose the section (family and size) in the list above the tools.",
            "Click where the columns stand: near the corner of two walls, the end of a beam or another column, the click lands on it.",
            "In Properties type the height of the column and the elevation of its foot.",
        ],
        figure: column,
    },
    Help {
        id: "Beam",
        steps: &[
            "Choose the section (family and size) in the list above the tools.",
            "Click the points of the beams: one beam is drawn for each segment.",
            "A beam is cut where it meets a column or another beam. In Properties type the elevation of its underside.",
        ],
        figure: beam,
    },
    Help {
        id: "Edit Structure",
        steps: &[
            "Select the columns or the beams, then start the tool.",
            "Choose what to change: the section, the height, the elevation or the rotation, and type the new value.",
            "The same values are changed in Properties, with the section list above the tools.",
        ],
        figure: edit_structure,
    },
    Help {
        id: "Structure Joints",
        steps: &[
            "Beams are cut where they meet a column or another beam as they are drawn.",
            "After moving, copying or erasing columns or beams, run this tool: it cuts them again.",
            "Nothing is asked: every column and beam of the drawing is redrawn.",
        ],
        figure: structure_joints,
    },
    Help {
        id: "Structure Takeoff",
        steps: &[
            "No selection needed: the whole drawing is counted.",
            "The steel columns and beams are listed by section with their count, length and mass.",
            "The slabs, the false ceilings, the stairs, the railings, the curtain walls and the roofs are counted too.",
            "It saves a workbook (.xlsx) or a text file.",
        ],
        figure: structure_takeoff,
    },
    Help {
        id: "Slab Opening",
        steps: &[
            "Select the slab, then click the corners of the opening and press Enter.",
            "The area of the opening is taken out of the area and the volume of the slab.",
            "Edit Slab changes its kind, its thickness and its elevation afterwards.",
        ],
        figure: slab_opening,
    },
];

/// Asks for the help of the tool whose label is `id` to open.
pub(super) fn request(ctx: &egui::Context, id: &'static str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(OPEN), id));
}

/// Shows the window asked for, if any.
pub(super) fn show(ctx: &egui::Context) {
    let Some(id) = ctx.data(|d| d.get_temp::<&'static str>(egui::Id::unique(OPEN))) else { return };
    let Some(help) = HELPS.iter().find(|h| h.id == id) else { return };
    let mut open = true;
    egui::Window::new(crate::i18n::t(help.id)).id(egui::Id::unique("hvac_help_window")).open(&mut open).collapsible(false).resizable(false).show(
        ctx,
        |ui| {
            ui.set_width(440.0);
            let (r, _) = ui.allocate_exact_size(vec2(440.0, 170.0), egui::Sense::hover());
            let t = Tokens::get();
            ui.painter().rect_filled(r, 4.0, t.chrome_dark);
            (help.figure)(&ui.painter().with_clip_rect(r), r.shrink(10.0));
            ui.add_space(8.0);
            for (i, step) in help.steps.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}.", i + 1));
                    ui.label(crate::i18n::t(step));
                });
            }
        },
    );
    if !open {
        ctx.data_mut(|d| d.remove::<&'static str>(egui::Id::unique(OPEN)));
    }
}

const DUCT: Color32 = Color32::from_rgb(0x2f, 0x86, 0xe8);
const DUCT_FILL: Color32 = Color32::from_rgb(0x1e, 0x33, 0x50);
const ZONE: Color32 = Color32::from_rgb(0xf0, 0xa1, 0x5f);
const GREY: Color32 = Color32::from_gray(150);

fn stroke() -> Stroke {
    Stroke::new(1.4, DUCT)
}

/// A horizontal duct from `x0` to `x1`, `w` wide, centred on `y`.
fn hduct(p: &Painter, x0: f32, x1: f32, y: f32, w: f32) {
    let r = Rect::from_min_max(pos2(x0, y - w / 2.0), pos2(x1, y + w / 2.0));
    p.rect_filled(r, 0.0, DUCT_FILL);
    p.rect_stroke(r, 0.0, stroke(), egui::StrokeKind::Inside);
}

fn vduct(p: &Painter, x: f32, y0: f32, y1: f32, w: f32) {
    let r = Rect::from_min_max(pos2(x - w / 2.0, y0), pos2(x + w / 2.0, y1));
    p.rect_filled(r, 0.0, DUCT_FILL);
    p.rect_stroke(r, 0.0, stroke(), egui::StrokeKind::Inside);
}

fn text(p: &Painter, at: egui::Pos2, s: &str, color: Color32) {
    p.text(at, egui::Align2::LEFT_CENTER, s, egui::FontId::proportional(11.0), color);
}

/// A terminal: a square with a cross.
fn terminal(p: &Painter, c: egui::Pos2) {
    let r = Rect::from_center_size(c, vec2(16.0, 16.0));
    p.rect_stroke(r, 0.0, Stroke::new(1.2, Color32::WHITE), egui::StrokeKind::Inside);
    p.line_segment([r.left_top(), r.right_bottom()], Stroke::new(1.0, Color32::WHITE));
    p.line_segment([r.right_top(), r.left_bottom()], Stroke::new(1.0, Color32::WHITE));
}

/// Diagonal hatching of `r`.
fn hatch(p: &Painter, r: Rect) {
    let s = Stroke::new(1.0, ZONE.gamma_multiply(0.7));
    let mut k = -r.height();
    while k < r.width() {
        let a = pos2(r.left() + k, r.bottom());
        let b = pos2(r.left() + k + r.height(), r.top());
        // Clip the line to the rectangle by the painter's own clip rect.
        p.with_clip_rect(r).line_segment([a, b], s);
        k += 9.0;
    }
}

fn riser(p: &Painter, r: Rect) {
    let c = r.center();
    let shaft = Rect::from_center_size(c, vec2(90.0, 60.0));
    p.rect_filled(shaft, 0.0, DUCT_FILL);
    p.rect_stroke(shaft, 0.0, stroke(), egui::StrokeKind::Inside);
    p.circle_stroke(c, 16.0, stroke());
    p.line_segment([c, c + vec2(70.0, -45.0)], Stroke::new(1.0, DUCT));
    text(p, c + vec2(72.0, -45.0), "Riser 1-Ø500-7000 m³/h", DUCT);
    p.arrow(c + vec2(-30.0, 28.0), vec2(0.0, -20.0), Stroke::new(1.2, DUCT));
}

fn duct(p: &Painter, r: Rect) {
    let y = r.center().y;
    hduct(p, r.left() + 40.0, r.right() - 60.0, y, 22.0);
    vduct(p, r.center().x, r.top() + 10.0, y, 16.0);
    let unit = Rect::from_center_size(pos2(r.left() + 20.0, y), vec2(40.0, 50.0));
    p.rect_stroke(unit, 0.0, Stroke::new(1.2, GREY), egui::StrokeKind::Inside);
    text(p, unit.center() - vec2(10.0, 0.0), "CTA", GREY);
    terminal(p, pos2(r.right() - 40.0, y));
    terminal(p, pos2(r.center().x, r.top() + 10.0));
    text(p, pos2(r.left() + 60.0, y - 24.0), "1", DUCT);
    text(p, pos2(r.center().x + 14.0, y - 30.0), "2", DUCT);
}

fn zone(p: &Painter, r: Rect) {
    let y = r.center().y;
    let z = Rect::from_min_max(pos2(r.center().x - 60.0, r.top()), pos2(r.center().x + 60.0, r.bottom()));
    p.rect_filled(z, 0.0, ZONE.gamma_multiply(0.18));
    hatch(p, z);
    p.rect_stroke(z, 0.0, Stroke::new(1.6, ZONE), egui::StrokeKind::Inside);
    // Seen from above the flat duct is wider than the round one: the same air in less height.
    hduct(p, r.left(), z.left(), y, 24.0);
    hduct(p, z.left(), z.right(), y, 42.0);
    hduct(p, z.right(), r.right(), y, 24.0);
    // The labels sit on a patch of the background, over the hatching.
    let patch = |at: egui::Pos2| p.rect_filled(Rect::from_min_size(at, vec2(66.0, 16.0)), 2.0, Tokens::get().chrome_dark);
    patch(pos2(z.left() + 4.0, r.top() + 4.0));
    text(p, pos2(z.left() + 8.0, r.top() + 12.0), "H 300 mm", ZONE);
    patch(pos2(z.left() + 4.0, y + 26.0));
    text(p, pos2(z.left() + 8.0, y + 34.0), "600×200", DUCT);
    text(p, pos2(r.left() + 6.0, y - 20.0), "Ø400", DUCT);
    text(p, pos2(r.right() - 44.0, y - 20.0), "Ø400", DUCT);
}

fn connect(p: &Painter, r: Rect) {
    let y = r.center().y + 20.0;
    hduct(p, r.left(), r.right(), y, 20.0);
    let t = pos2(r.center().x, r.top() + 22.0);
    terminal(p, t);
    // The flexible duct: a dashed line from the terminal to the duct.
    let mut yy = t.y + 10.0;
    while yy < y - 10.0 {
        p.line_segment([pos2(t.x, yy), pos2(t.x, (yy + 5.0).min(y - 10.0))], Stroke::new(1.6, DUCT));
        yy += 9.0;
    }
    text(p, t + vec2(16.0, 6.0), "flexible", DUCT);
}

fn distribute(p: &Painter, r: Rect) {
    // An L-shaped room seen from above with a grid of units: the cells whose centre is outside it stay empty.
    let room = Rect::from_min_max(pos2(r.left() + 20.0, r.top() + 8.0), pos2(r.right() - 20.0, r.bottom() - 8.0));
    let s = Stroke::new(1.4, Color32::from_gray(190));
    let notch = Rect::from_min_max(pos2(room.center().x + 24.0, room.top()), pos2(room.right(), room.center().y - 6.0));
    let outline = [
        room.left_top(),
        notch.left_top(),
        notch.left_bottom(),
        pos2(room.right(), notch.bottom()),
        room.right_bottom(),
        room.left_bottom(),
        room.left_top(),
    ];
    p.add(egui::Shape::line(outline.to_vec(), s));
    let (rows, cols) = (3, 4);
    for row in 0..rows {
        for col in 0..cols {
            let c =
                pos2(room.left() + room.width() * (col as f32 + 0.5) / cols as f32, room.top() + room.height() * (row as f32 + 0.5) / rows as f32);
            if notch.contains(c) {
                p.circle_stroke(c, 4.0, Stroke::new(1.0, Color32::from_gray(90)));
            } else {
                terminal(p, c);
            }
        }
    }
    text(p, pos2(room.left() + 6.0, room.bottom() + 1.0), "2x3, 6, 3x4…", Color32::from_gray(190));
}

fn points_list(p: &Painter, r: Rect) {
    // A controller in the middle, the sensors and the actuators wired to it, and the bus that goes on.
    let c = r.center();
    let ctrl = Rect::from_center_size(c, vec2(70.0, 40.0));
    let white = Color32::from_gray(200);
    p.rect_stroke(ctrl, 0.0, Stroke::new(1.4, white), egui::StrokeKind::Inside);
    text(p, c + vec2(-14.0, 0.0), "AU1", white);
    for (dx, dy, circle_shape) in [(-100.0, -30.0, true), (-100.0, 30.0, true), (100.0, -30.0, false), (100.0, 30.0, false)] {
        let d = c + vec2(dx, dy);
        p.line_segment(
            [ctrl.center() + vec2(dx.signum() * 35.0, dy / 3.0), d + vec2(-dx.signum() * 12.0, 0.0)],
            Stroke::new(1.0, Color32::from_gray(120)),
        );
        if circle_shape {
            p.circle_stroke(d, 9.0, Stroke::new(1.2, white));
        } else {
            p.rect_stroke(Rect::from_center_size(d, vec2(18.0, 14.0)), 3.0, Stroke::new(1.2, white), egui::StrokeKind::Inside);
        }
    }
    text(p, pos2(r.left() + 6.0, r.bottom() - 8.0), "AI  AI              AO  AO", Color32::from_gray(150));
    p.line_segment([pos2(c.x - 10.0, ctrl.top()), pos2(c.x - 10.0, r.top() + 4.0)], Stroke::new(1.6, Color32::from_rgb(0x90, 0x60, 0xd0)));
}

fn size(p: &Painter, r: Rect) {
    let y = r.center().y;
    hduct(p, r.left(), r.center().x + 20.0, y, 26.0);
    hduct(p, r.center().x + 20.0, r.right(), y, 16.0);
    vduct(p, r.center().x - 40.0, r.top() + 12.0, y - 13.0, 14.0);
    text(p, pos2(r.left() + 4.0, y + 24.0), "Ø315  1400 m³/h", DUCT);
    text(p, pos2(r.center().x + 26.0, y + 22.0), "Ø200  400 m³/h", DUCT);
    text(p, pos2(r.center().x - 30.0, r.top() + 12.0), "Ø200  350 m³/h", DUCT);
    terminal(p, pos2(r.right() - 10.0, y));
}

fn crossing(p: &Painter, r: Rect) {
    let c = r.center();
    vduct(p, c.x, r.top(), r.bottom(), 36.0);
    hduct(p, r.left(), c.x - 70.0, c.y, 22.0);
    hduct(p, c.x + 70.0, r.right(), c.y, 22.0);
    // Four 45° elbows, each a shallow arc bulging toward the middle of the jog.
    for (i, dx) in [-70.0_f32, -34.0, 34.0, 70.0].into_iter().enumerate() {
        let x = c.x + dx;
        let bulge = if i < 2 { 5.0 } else { -5.0 };
        let pts: Vec<_> = (0..=8)
            .map(|k| {
                let t = k as f32 / 8.0;
                let off = bulge * (1.0 - (2.0 * t - 1.0).powi(2));
                pos2(x + off, c.y - 11.0 + 22.0 * t)
            })
            .collect();
        p.add(egui::Shape::line(pts, stroke()));
    }
    p.line_segment([pos2(c.x - 34.0, c.y - 11.0), pos2(c.x + 34.0, c.y - 11.0)], stroke());
    p.line_segment([pos2(c.x - 34.0, c.y + 11.0), pos2(c.x + 34.0, c.y + 11.0)], stroke());
    text(p, pos2(c.x + 24.0, r.top() + 14.0), "-458", DUCT);
}

fn quantities(p: &Painter, r: Rect) {
    let rows = [("Ø200", "12", "34,5 m"), ("Ø315", "5", "18,2 m"), ("600×200", "2", "6,0 m")];
    let x = r.left() + 20.0;
    text(p, pos2(x, r.top() + 14.0), "Section", GREY);
    text(p, pos2(x + 120.0, r.top() + 14.0), "Count", GREY);
    text(p, pos2(x + 200.0, r.top() + 14.0), "Length", GREY);
    for (i, (a, b, c)) in rows.into_iter().enumerate() {
        let y = r.top() + 40.0 + 28.0 * i as f32;
        p.hline(r.x_range(), y - 14.0, Stroke::new(1.0, Color32::from_gray(70)));
        text(p, pos2(x, y), a, Color32::WHITE);
        text(p, pos2(x + 120.0, y), b, Color32::WHITE);
        text(p, pos2(x + 200.0, y), c, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn every_tool_has_a_help_window_that_draws_and_closes() {
        assert_eq!(HELPS.len(), 32);
        for help in HELPS {
            let mut h = Harness::builder().with_size(vec2(700.0, 500.0)).build_ui(|ui| {
                show(ui.ctx());
            });
            request(&h.ctx, help.id);
            h.run_steps(3);
            assert!(h.query_by_label(help.steps[0]).is_some(), "{}: the first step is shown", help.id);
            // Its window is gone once closed.
            h.ctx.data_mut(|d| d.remove::<&'static str>(egui::Id::unique(OPEN)));
            h.run_steps(2);
            assert!(h.query_by_label(help.steps[0]).is_none(), "{}: closed", help.id);
        }
    }
}

#[cfg(test)]
mod shots {
    use super::*;
    use egui_kittest::Harness;

    /// Writes each drawing to the directory in CADCRAFT_HELP_SHOTS, when it is set (a look at the real rendering).
    #[test]
    fn drawings_can_be_saved_to_look_at() {
        let Some(dir) = std::env::var_os("CADCRAFT_HELP_SHOTS") else { return };
        for help in HELPS {
            let mut h = Harness::builder().with_size(vec2(480.0, 380.0)).build_ui(|ui| {
                show(ui.ctx());
            });
            request(&h.ctx, help.id);
            h.run_steps(3);
            if let Ok(img) = h.render() {
                let _ = img.save(std::path::Path::new(&dir).join(format!("{}.png", help.id.replace(' ', "_"))));
            }
        }
    }
}

const PIPE_BLUE: Color32 = Color32::from_rgb(0x1e, 0x6f, 0xd9);
const PIPE_RED: Color32 = Color32::from_rgb(0xe0, 0x24, 0x24);
const PIPE_ORANGE: Color32 = Color32::from_rgb(0xe6, 0x7e, 0x22);
const WALL: Color32 = Color32::from_gray(190);

/// A pipe seen from above: two lines.
fn pipe_line(p: &Painter, a: egui::Pos2, b: egui::Pos2, color: Color32) {
    let d = (b - a).normalized();
    let n = vec2(-d.y, d.x) * 2.5;
    let s = Stroke::new(1.3, color);
    p.line_segment([a + n, b + n], s);
    p.line_segment([a - n, b - n], s);
}

fn unit_box(p: &Painter, c: egui::Pos2, size: egui::Vec2, label: &str) {
    let r = Rect::from_center_size(c, size);
    p.rect_stroke(r, 0.0, Stroke::new(1.2, GREY), egui::StrokeKind::Inside);
    p.text(c, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), GREY);
}

fn piping(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    unit_box(p, at(0.07, 0.5), vec2(46.0, 54.0), "GE");
    pipe_line(p, at(0.14, 0.34), at(0.78, 0.34), PIPE_BLUE);
    pipe_line(p, at(0.14, 0.66), at(0.78, 0.66), PIPE_BLUE.gamma_multiply(0.6));
    pipe_line(p, at(0.5, 0.34), at(0.5, 0.05), PIPE_BLUE);
    unit_box(p, at(0.88, 0.34), vec2(44.0, 34.0), "UI");
    unit_box(p, at(0.88, 0.66), vec2(44.0, 34.0), "UI");
    text(p, at(0.2, 0.2), "supply", PIPE_BLUE);
    text(p, at(0.2, 0.82), "return", PIPE_BLUE.gamma_multiply(0.7));
    text(p, at(0.53, 0.1), "branch from the end of a run", GREY);
}

fn size_pipes(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    unit_box(p, at(0.07, 0.5), vec2(46.0, 54.0), "GE");
    pipe_line(p, at(0.14, 0.5), at(0.5, 0.5), PIPE_BLUE);
    pipe_line(p, at(0.5, 0.5), at(0.5, 0.18), PIPE_BLUE);
    pipe_line(p, at(0.5, 0.5), at(0.5, 0.82), PIPE_BLUE);
    pipe_line(p, at(0.5, 0.18), at(0.8, 0.18), PIPE_BLUE);
    pipe_line(p, at(0.5, 0.82), at(0.8, 0.82), PIPE_BLUE);
    unit_box(p, at(0.89, 0.18), vec2(44.0, 28.0), "30 kW");
    unit_box(p, at(0.89, 0.82), vec2(44.0, 28.0), "70 kW");
    text(p, at(0.15, 0.34), "source  DN80", PIPE_BLUE);
    text(p, at(0.53, 0.34), "DN50", PIPE_BLUE);
    text(p, at(0.53, 0.68), "DN65", PIPE_BLUE);
    text(p, at(0.15, 0.7), "100 kW", GREY);
}

fn pipe_riser(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let shaft = Rect::from_center_size(at(0.72, 0.5), vec2(110.0, 70.0));
    p.rect_filled(shaft, 0.0, DUCT_FILL);
    p.rect_stroke(shaft, 0.0, Stroke::new(1.4, GREY), egui::StrokeKind::Inside);
    for (k, color) in [PIPE_BLUE, PIPE_BLUE.gamma_multiply(0.6), PIPE_RED, PIPE_ORANGE].into_iter().enumerate() {
        let c = shaft.left_top() + vec2(18.0 + 25.0 * k as f32, 35.0);
        p.circle_stroke(c, 8.0, Stroke::new(1.4, color));
        p.circle_filled(c, 1.8, color);
    }
    pipe_line(p, at(0.05, 0.4), pos2(shaft.left() + 12.0, at(0.0, 0.4).y), PIPE_BLUE);
    pipe_line(p, at(0.05, 0.62), pos2(shaft.left() + 12.0, at(0.0, 0.62).y), PIPE_BLUE.gamma_multiply(0.6));
    text(p, at(0.05, 0.2), "pipes end in the shaft", GREY);
    text(p, shaft.left_bottom() + vec2(0.0, 12.0), "CT1  CHW-S/R  HW  REF", GREY);
}

fn name_networks(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let colours = [Color32::from_rgb(0x00, 0x72, 0xb2), Color32::from_rgb(0xe6, 0x9f, 0x00), Color32::from_rgb(0x00, 0x9e, 0x73)];
    for (k, (c, name)) in colours.into_iter().zip(["CHW1", "CHW2", "CHW3"]).enumerate() {
        let y = 0.2 + 0.3 * k as f32;
        unit_box(p, at(0.06, y), vec2(34.0, 28.0), "GE");
        pipe_line(p, at(0.12, y - 0.04), at(0.8, y - 0.04), c);
        pipe_line(p, at(0.12, y + 0.06), at(0.8, y + 0.06), c.gamma_multiply(0.7));
        unit_box(p, at(0.88, y), vec2(34.0, 28.0), "UI");
        text(p, at(0.4, y - 0.12), name, c);
    }
}

fn network_legend(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let frame = Rect::from_min_max(at(0.18, 0.04), at(0.82, 0.96));
    p.rect_stroke(frame, 0.0, Stroke::new(1.0, GREY), egui::StrokeKind::Inside);
    text(p, at(0.22, 0.14), "Networks", Color32::WHITE);
    let colours = [Color32::from_rgb(0x00, 0x72, 0xb2), Color32::from_rgb(0xe6, 0x9f, 0x00), Color32::from_rgb(0x00, 0x9e, 0x73)];
    for (k, (c, row)) in colours.into_iter().zip(["CHW1  CHW-S/R  42 m", "CHW2  CHW-S/R  57 m", "COND1  COND  18 m"]).enumerate() {
        let y = 0.36 + 0.22 * k as f32;
        pipe_line(p, at(0.22, y), at(0.38, y), c);
        text(p, at(0.42, y), row, c);
    }
}

fn library(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    pipe_line(p, at(0.05, 0.5), at(0.36, 0.5), PIPE_BLUE);
    pipe_line(p, at(0.64, 0.5), at(0.95, 0.5), PIPE_BLUE);
    // A valve in the line, the pipe cut where it sits.
    let c = at(0.5, 0.5);
    let s = Stroke::new(1.4, Color32::from_rgb(0xe6, 0xb4, 0x3c));
    p.add(egui::Shape::closed_line(vec![c + vec2(-16.0, -12.0), c + vec2(16.0, 12.0), c + vec2(16.0, -12.0), c + vec2(-16.0, 12.0)], s));
    text(p, at(0.42, 0.22), "valve on a pipe: cuts it", GREY);
    // A pump and a vessel, put anywhere.
    let teal = Stroke::new(1.4, Color32::from_rgb(0x3f, 0xb8, 0xa8));
    p.circle_stroke(at(0.2, 0.82), 12.0, teal);
    p.circle_stroke(at(0.8, 0.82), 12.0, teal);
    p.line_segment([at(0.8, 0.82) - vec2(12.0, 0.0), at(0.8, 0.82) + vec2(12.0, 0.0)], teal);
    p.arrow(at(0.5, 0.95), vec2(0.0, -14.0), Stroke::new(1.0, GREY));
}

fn check_refrigerant(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    unit_box(p, at(0.07, 0.5), vec2(46.0, 54.0), "VRV");
    pipe_line(p, at(0.14, 0.5), at(0.36, 0.5), PIPE_ORANGE);
    let bx = Rect::from_center_size(at(0.46, 0.5), vec2(48.0, 24.0));
    p.rect_stroke(bx, 0.0, Stroke::new(1.3, PIPE_ORANGE), egui::StrokeKind::Inside);
    for y in [0.16, 0.5, 0.84] {
        pipe_line(p, at(0.52, 0.5), at(0.74, y), PIPE_ORANGE);
        unit_box(p, at(0.88, y), vec2(44.0, 26.0), "UI 25");
    }
    text(p, at(0.2, 0.86), "outlet: 5 units, index 140", GREY);
}

fn convert_openings(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let wall = |x0: f32, x1: f32| {
        p.rect_filled(Rect::from_min_max(at(x0, 0.42), at(x1, 0.58)), 0.0, WALL.gamma_multiply(0.35));
        p.rect_stroke(Rect::from_min_max(at(x0, 0.42), at(x1, 0.58)), 0.0, Stroke::new(1.2, WALL), egui::StrokeKind::Inside);
    };
    wall(0.05, 0.4);
    wall(0.6, 0.95);
    // The old block, then the opening that replaces it.
    p.line_segment([at(0.4, 0.5), at(0.6, 0.5)], Stroke::new(1.0, GREY));
    // The swing of the door, a quarter of a circle.
    let (c, rad) = (at(0.4, 0.5), 0.2 * r.width());
    let swing: Vec<egui::Pos2> = (0..=12)
        .map(|i| {
            let a = std::f32::consts::FRAC_PI_2 * i as f32 / 12.0;
            c + vec2(rad * a.cos(), -rad * a.sin())
        })
        .collect();
    p.add(egui::Shape::line(swing, Stroke::new(1.0, GREY)));
    text(p, at(0.36, 0.2), "block  >  opening", GREY);
    text(p, at(0.3, 0.86), "old block kept on a frozen layer", GREY);
}

fn wall_elevation(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let wall = Rect::from_min_max(at(0.1, 0.45), at(0.9, 0.95));
    p.rect_stroke(wall, 0.0, Stroke::new(1.3, WALL), egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(at(0.2, 0.55), at(0.34, 0.95)), 0.0, Stroke::new(1.2, DUCT), egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(at(0.55, 0.6), at(0.78, 0.8)), 0.0, Stroke::new(1.2, DUCT), egui::StrokeKind::Inside);
    // The plan above it.
    p.rect_filled(Rect::from_min_max(at(0.1, 0.1), at(0.9, 0.22)), 0.0, WALL.gamma_multiply(0.35));
    p.rect_stroke(Rect::from_min_max(at(0.1, 0.1), at(0.9, 0.22)), 0.0, Stroke::new(1.2, WALL), egui::StrokeKind::Inside);
    p.arrow(at(0.5, 0.26), vec2(0.0, 18.0), Stroke::new(1.0, GREY));
}

fn wall_junctions(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let fill = WALL.gamma_multiply(0.35);
    let s = Stroke::new(1.3, WALL);
    // An L, a T and a cross, joined without a line across.
    for (cx, kind) in [(0.17, 0), (0.5, 1), (0.83, 2)] {
        let c = at(cx, 0.5);
        let h = |x0: f32, x1: f32| Rect::from_min_max(c + vec2(x0, -7.0), c + vec2(x1, 7.0));
        let v = |y0: f32, y1: f32| Rect::from_min_max(c + vec2(-7.0, y0), c + vec2(7.0, y1));
        let parts: Vec<Rect> = match kind {
            0 => vec![h(-40.0, 7.0), v(-7.0, 40.0)],
            1 => vec![h(-40.0, 40.0), v(-7.0, 40.0)],
            _ => vec![h(-40.0, 40.0), v(-40.0, 40.0)],
        };
        for part in &parts {
            p.rect_filled(*part, 0.0, fill);
        }
        for part in parts {
            p.rect_stroke(part, 0.0, s, egui::StrokeKind::Inside);
        }
        // The joint: the middle painted over the lines that crossed it.
        p.circle_filled(c, 4.5, Tokens::get().chrome_dark);
    }
}

fn corner_window(p: &Painter, r: Rect) {
    let at = |x: f32, y: f32| pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let fill = WALL.gamma_multiply(0.35);
    let hwall = Rect::from_min_max(at(0.2, 0.72), at(0.8, 0.86));
    let vwall = Rect::from_min_max(at(0.66, 0.1), at(0.8, 0.86));
    for w in [hwall, vwall] {
        p.rect_filled(w, 0.0, fill);
        p.rect_stroke(w, 0.0, Stroke::new(1.3, WALL), egui::StrokeKind::Inside);
    }
    // A window on each wall, from the corner.
    let win = Stroke::new(2.0, DUCT);
    p.line_segment([at(0.36, 0.79), at(0.66, 0.79)], win);
    p.line_segment([at(0.73, 0.2), at(0.73, 0.72)], win);
    text(p, at(0.24, 0.56), "click near the corner", GREY);
}

// ---------------------------------------------------------------------------------------------------------------
// The design and structure tools.
// ---------------------------------------------------------------------------------------------------------------

const STEEL: Color32 = Color32::from_rgb(0xd0, 0x50, 0x50);
const FRAME: Color32 = Color32::from_rgb(0x50, 0x90, 0xa0);
const ROOF: Color32 = Color32::from_rgb(0xc0, 0x70, 0x58);

fn fx(r: Rect) -> impl Fn(f32, f32) -> egui::Pos2 {
    move |x, y| pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

/// A steel column seen from above: an I, filled.
fn column_mark(p: &Painter, c: egui::Pos2, k: f32) {
    let (w, f) = (7.0 * k, 2.2 * k);
    p.rect_filled(Rect::from_center_size(c, vec2(2.0 * w, 2.0 * w)), 0.0, STEEL.gamma_multiply(0.3));
    for dy in [-w + f / 2.0, w - f / 2.0] {
        p.rect_filled(Rect::from_center_size(c + vec2(0.0, dy), vec2(2.0 * w, f)), 0.0, STEEL);
    }
    p.line_segment([c + vec2(0.0, -w), c + vec2(0.0, w)], Stroke::new(1.3, STEEL));
}

fn ceiling_grid(p: &Painter, r: Rect) {
    let at = fx(r);
    let room = Rect::from_min_max(at(0.15, 0.12), at(0.85, 0.88));
    p.rect_stroke(room, 0.0, Stroke::new(1.4, WALL), egui::StrokeKind::Inside);
    for i in 1..6 {
        let x = room.left() + room.width() * i as f32 / 6.0;
        p.line_segment([pos2(x, room.top()), pos2(x, room.bottom())], Stroke::new(1.0, GREY));
    }
    for i in 1..4 {
        let y = room.top() + room.height() * i as f32 / 4.0;
        p.line_segment([pos2(room.left(), y), pos2(room.right(), y)], Stroke::new(1.0, GREY));
    }
    terminal(p, pos2(room.left() + room.width() * 0.25, room.top() + room.height() * 0.375));
    text(p, at(0.2, 0.96), "tile 600 x 600", GREY);
}

fn stair(p: &Painter, r: Rect) {
    let at = fx(r);
    let flight = Rect::from_min_max(at(0.38, 0.1), at(0.62, 0.9));
    p.rect_stroke(flight, 0.0, Stroke::new(1.4, ZONE), egui::StrokeKind::Inside);
    for i in 1..10 {
        let y = flight.top() + flight.height() * i as f32 / 10.0;
        p.line_segment([pos2(flight.left(), y), pos2(flight.right(), y)], Stroke::new(1.0, ZONE));
    }
    let (a, b) = (at(0.5, 0.86), at(0.5, 0.2));
    let white = Stroke::new(1.4, Color32::WHITE);
    p.line_segment([a, b], white);
    p.line_segment([b, b + vec2(-5.0, 8.0)], white);
    p.line_segment([b, b + vec2(5.0, 8.0)], white);
    text(p, at(0.66, 0.5), "rise 3000 / 17 risers", GREY);
}

fn railing(p: &Painter, r: Rect) {
    let at = fx(r);
    let pts = [at(0.1, 0.7), at(0.45, 0.7), at(0.45, 0.3), at(0.9, 0.3)];
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(3.0, ZONE));
    }
    for q in [pts[0], at(0.275, 0.7), pts[1], at(0.45, 0.5), pts[2], at(0.675, 0.3), pts[3]] {
        p.rect_filled(Rect::from_center_size(q, vec2(7.0, 7.0)), 0.0, Color32::WHITE);
    }
    text(p, at(0.1, 0.9), "post spacing", GREY);
}

fn curtain_wall(p: &Painter, r: Rect) {
    let at = fx(r);
    let (a, b) = (at(0.1, 0.5), at(0.9, 0.5));
    p.line_segment([a, b], Stroke::new(6.0, FRAME));
    p.line_segment([a, b], Stroke::new(1.0, Color32::from_rgb(0x80, 0xd0, 0xe8)));
    for i in 0..=4 {
        let x = a.x + (b.x - a.x) * i as f32 / 4.0;
        p.line_segment([pos2(x, a.y - 9.0), pos2(x, a.y + 9.0)], Stroke::new(1.6, FRAME));
    }
    text(p, at(0.3, 0.8), "mullion every 1500 mm", GREY);
}

fn building_section(p: &Painter, r: Rect) {
    let at = fx(r);
    let s = Stroke::new(1.3, WALL);
    p.line_segment([at(0.05, 0.12), at(0.95, 0.12)], Stroke::new(1.3, ZONE));
    text(p, at(0.03, 0.05), "A", ZONE);
    text(p, at(0.94, 0.05), "A", ZONE);
    for (x0, x1) in [(0.2, 0.24), (0.76, 0.8)] {
        let w = Rect::from_min_max(at(x0, 0.3), at(x1, 0.8));
        p.rect_filled(w, 0.0, WALL.gamma_multiply(0.5));
        p.rect_stroke(w, 0.0, s, egui::StrokeKind::Inside);
    }
    let slab = Rect::from_min_max(at(0.2, 0.8), at(0.8, 0.88));
    p.rect_filled(slab, 0.0, WALL.gamma_multiply(0.5));
    p.rect_stroke(slab, 0.0, s, egui::StrokeKind::Inside);
    p.line_segment([at(0.2, 0.46), at(0.8, 0.46)], Stroke::new(1.0, GREY));
    text(p, at(0.4, 0.62), "+2.60", GREY);
    text(p, at(0.08, 0.8), "+0.00", GREY);
}

fn roof(p: &Painter, r: Rect) {
    let at = fx(r);
    let outline = Rect::from_min_max(at(0.12, 0.12), at(0.88, 0.88));
    p.rect_stroke(outline, 0.0, Stroke::new(1.4, ROOF), egui::StrokeKind::Inside);
    let (y, h) = (outline.center().y, outline.height() / 2.0);
    let (x0, x1) = (outline.left() + h, outline.right() - h);
    let s = Stroke::new(1.2, ROOF);
    p.line_segment([pos2(x0, y), pos2(x1, y)], s);
    for (cx, ex) in [(x0, outline.left()), (x1, outline.right())] {
        p.line_segment([pos2(cx, y), pos2(ex, outline.top())], s);
        p.line_segment([pos2(cx, y), pos2(ex, outline.bottom())], s);
    }
    for dy in [-0.5f32, 0.5] {
        let c = pos2(outline.center().x, y + dy * h);
        let tip = c + vec2(0.0, dy * 28.0);
        p.line_segment([c, tip], Stroke::new(1.2, Color32::WHITE));
        p.line_segment([tip, tip + vec2(-4.0, -dy * 6.0)], Stroke::new(1.2, Color32::WHITE));
        p.line_segment([tip, tip + vec2(4.0, -dy * 6.0)], Stroke::new(1.2, Color32::WHITE));
    }
    text(p, at(0.14, 0.96), "ridge, hips and falls", GREY);
}

fn column(p: &Painter, r: Rect) {
    let at = fx(r);
    column_mark(p, at(0.3, 0.45), 2.2);
    column_mark(p, at(0.7, 0.45), 2.2);
    text(p, at(0.2, 0.82), "C1  HEA 200  h=3.00 m", STEEL);
}

fn beam(p: &Painter, r: Rect) {
    let at = fx(r);
    let (a, b) = (at(0.25, 0.45), at(0.75, 0.45));
    p.line_segment([a, b], Stroke::new(3.0, DUCT));
    column_mark(p, a, 1.6);
    column_mark(p, b, 1.6);
    text(p, at(0.3, 0.7), "B1  IPE 240", DUCT);
}

fn edit_structure(p: &Painter, r: Rect) {
    let at = fx(r);
    column_mark(p, at(0.3, 0.4), 1.8);
    p.line_segment([at(0.3, 0.4), at(0.75, 0.4)], Stroke::new(3.0, DUCT));
    text(p, at(0.2, 0.72), "HEA 200  ->  HEB 240", STEEL);
}

fn structure_joints(p: &Painter, r: Rect) {
    let at = fx(r);
    let c = at(0.5, 0.45);
    p.line_segment([at(0.1, 0.45), c - vec2(10.0, 0.0)], Stroke::new(3.0, DUCT));
    p.line_segment([c + vec2(10.0, 0.0), at(0.9, 0.45)], Stroke::new(3.0, DUCT));
    column_mark(p, c, 1.4);
    text(p, at(0.25, 0.8), "the beam stops at the column", GREY);
}

fn structure_takeoff(p: &Painter, r: Rect) {
    let at = fx(r);
    let t = Rect::from_min_max(at(0.15, 0.12), at(0.85, 0.82));
    p.rect_stroke(t, 0.0, Stroke::new(1.2, GREY), egui::StrokeKind::Inside);
    for i in 1..5 {
        let y = t.top() + t.height() * i as f32 / 5.0;
        p.line_segment([pos2(t.left(), y), pos2(t.right(), y)], Stroke::new(1.0, GREY));
    }
    for i in 1..4 {
        let x = t.left() + t.width() * i as f32 / 4.0;
        p.line_segment([pos2(x, t.top()), pos2(x, t.bottom())], Stroke::new(1.0, GREY));
    }
    text(p, at(0.17, 0.93), "section  count  length  mass", GREY);
}

fn slab_opening(p: &Painter, r: Rect) {
    let at = fx(r);
    let slab = Rect::from_min_max(at(0.15, 0.1), at(0.85, 0.9));
    p.rect_filled(slab, 0.0, WALL.gamma_multiply(0.2));
    p.rect_stroke(slab, 0.0, Stroke::new(1.4, WALL), egui::StrokeKind::Inside);
    let hole = Rect::from_min_max(at(0.4, 0.35), at(0.6, 0.65));
    p.rect_filled(hole, 0.0, Tokens::get().chrome_dark);
    p.rect_stroke(hole, 0.0, Stroke::new(1.4, DUCT), egui::StrokeKind::Inside);
    p.line_segment([hole.left_top(), hole.right_bottom()], Stroke::new(1.0, DUCT));
    p.line_segment([hole.right_top(), hole.left_bottom()], Stroke::new(1.0, DUCT));
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn every_help_has_steps_a_translation_and_a_figure_that_draws() {
        let mut ids: Vec<&str> = HELPS.iter().map(|h| h.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "one help for each tool");
        for h in HELPS {
            assert!((3..=5).contains(&h.steps.len()), "{}: {} steps", h.id, h.steps.len());
            for step in h.steps {
                assert_ne!(crate::i18n::tr("fr", step), *step, "{}: not translated into French: {step}", h.id);
                assert_ne!(crate::i18n::tr("zh", step), *step, "{}: not translated into Chinese: {step}", h.id);
            }
        }
        // Every figure draws without panicking, in a box of the size the window gives it.
        let mut hn = egui_kittest::Harness::new_ui(|ui| {
            for h in HELPS {
                let (r, _) = ui.allocate_exact_size(vec2(440.0, 170.0), egui::Sense::hover());
                (h.figure)(&ui.painter().with_clip_rect(r), r.shrink(10.0));
            }
        });
        hn.run_steps(2);
    }

    #[test]
    fn the_complex_tools_of_the_tabs_have_a_help() {
        let have = |id: &str| HELPS.iter().any(|h| h.id == id);
        for id in [
            "Piping",
            "Size Pipes",
            "Pipe Riser",
            "Name Networks",
            "Network Legend",
            "Library…",
            "Check Refrigerant",
            "Convert to Openings",
            "Wall Elevation",
            "Wall Junctions",
            "Corner Window",
            "Distribute in a Space",
            "Points List",
            "Ceiling Grid",
            "Stair",
            "Railing",
            "Curtain Wall",
            "Roof",
            "Building Section",
            "Column",
            "Beam",
            "Edit Structure",
            "Structure Joints",
            "Structure Takeoff",
            "Slab Opening",
        ] {
            assert!(have(id), "{id}");
        }
    }
}
