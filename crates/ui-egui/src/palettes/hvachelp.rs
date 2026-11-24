//! The "i" beside an HVAC tool: a window with a drawing of what the tool does (seen from above, as
//! on the plan) and the steps to follow. The drawings are made here, from lines and arcs.

use egui::{Color32, Painter, Rect, Stroke, pos2, vec2};

use crate::theme::Tokens;

/// One tool's help: its English name (the tool's label), the steps, and its drawing.
struct Help {
    id: &'static str,
    steps: &'static [&'static str],
    /// Its drawings, each with a caption ("" for none): most tools have one.
    figures: &'static [(&'static str, fn(&Painter, Rect))],
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
        figures: &[("", points_list)],
    },
    Help {
        id: "Distribute in a Space",
        steps: &[
            "Start the tool (air terminals) or Distribute Units in a Space (indoor units), then select the space.",
            "Type how many: 6 spreads six in the most even grid the room's shape allows; 2x3 makes 2 rows by 3 columns.",
            "A grid cell whose centre is outside the space gets nothing, so an L-shaped room is filled by its two arms.",
            "Terminals share the space's flow; units get their model's power. Edit one afterwards, or move it by its grip.",
        ],
        figures: &[("", distribute)],
    },
    Help {
        id: "Riser",
        steps: &[
            "Start the tool and click the corners of the shaft, then press Enter.",
            "Type the flow that comes from the other floors in Properties.",
            "Size Ducts gives the riser its section; the arrow shows if the air goes up or down.",
        ],
        figures: &[("", riser)],
    },
    Help {
        id: "Duct",
        steps: &[
            "Click the points of the run: one duct is drawn per segment.",
            "Start on a unit's collar, end on a terminal; start on another duct to make a tee.",
            "Leave the direction on auto: Size Ducts finds supply or extract from the terminals the run reaches.",
        ],
        figures: &[("", duct)],
    },
    Help {
        id: "Height Zone",
        steps: &[
            "Click the corners of the outline where ducts have less height (under a beam, a low ceiling), seen from above, then press Enter.",
            "Type the height available for the ducts, in mm: the void under the beam. Sizing keeps a margin back for the hangers (100 mm by default), so 250 mm available gives a duct 150 mm high.",
            "The altitude of the underside is optional: ducts under the zone sit at that altitude, and a step (two 45° elbows) is drawn between two runs at other altitudes as soon as they are laid.",
            "As soon as a duct is laid across the zone it is cut at its edges and the part inside is made flat: a duct too tall for the height available becomes rectangular, as high as allowed and as wide as it takes to keep the same air section. A section you typed is changed the same way.",
            "Size Ducts, later, sizes each piece for its flow in the height available (a network: start on a unit's collar, end on a terminal or a riser); a duct that leads to none keeps the flat section it was given.",
            "The altitude of the zone's underside is given to the pieces inside, unless they have their own, and where two runs are at other altitudes the step (two 45° elbows) is drawn at once, with no sizing.",
            "To change the zone afterwards, select it and edit its height or its altitude in Properties, then run Size Ducts again.",
        ],
        figures: &[
            ("1. Draw the outline, like a room", zone_plan),
            ("2. Why 300 mm: the round duct does not fit", zone_section),
            ("3. What Size Ducts does", zone),
            ("4. Option: an altitude, and the steps make themselves", zone_altitude),
        ],
    },
    Help {
        id: "Connect Terminals",
        steps: &[
            "No selection needed: every terminal that no duct reaches is taken.",
            "A flexible duct is drawn from each one to the nearest duct, 3 m at most.",
            "Terminals too far from any duct are listed and left alone.",
        ],
        figures: &[("", connect)],
    },
    Help {
        id: "Size Ducts",
        steps: &[
            "Run it when the ducts and terminals are drawn; it reads each network from its unit or riser.",
            "Each duct gets its section, and its flow and velocity are written on it.",
            "Run it again after any change: sections and flows are recalculated.",
        ],
        figures: &[("", size)],
    },
    Help {
        id: "Duct Crossing",
        steps: &[
            "Click the two ducts that cross.",
            "Choose which one passes under or over the other.",
            "It is drawn broken, or with four 45° elbows when the duct has to go round the other.",
        ],
        figures: &[("", crossing)],
    },
    Help {
        id: "HVAC Quantities",
        steps: &[
            "Draw one drawing per level, then open this tool on the level.",
            "The tables count ducts by section, fittings, terminals, units, risers and equipment.",
            "The spreadsheet button saves them for Excel; each level makes its own file.",
        ],
        figures: &[("", quantities)],
    },
    Help {
        id: "Piping",
        steps: &[
            "Choose a service (chilled water, hot water, change-over, condensate, refrigerant), then click the points of the run: one pipe is drawn per segment, seen from above.",
            "At the command line: Service, Leg (supply or return), Diameter (inches for refrigerant: 3/8, 1 1/8) and Insulation. A refrigerant link draws its liquid and gas tubes as one line.",
            "Each service has its own colour. To size a network, mark the run that starts at the source, type the loads at the free ends, then use Size Pipes.",
            "A pipe is joined to another only where their ends meet: start a branch at the end of a run.",
        ],
        figures: &[("", piping)],
    },
    Help {
        id: "Size Pipes",
        steps: &[
            "It sizes chilled water, hot water, change-over and condensate. Refrigerant links are not sized: type their diameters in inches.",
            "Each network needs a source: tick \"Starts at the source\" on its first run, or start the run on a chiller or in the shaft of a pipe riser.",
            "Type the load at each free end (kW, l/h for a drain), or end the run in a unit or an air handling unit that has a power: its coil counts.",
            "Run it: each pipe gets its size, flow and velocity, and the pressure lost on the most unfavourable circuit is told. A size you typed is checked, not changed. Run it again after any change.",
        ],
        figures: &[("", size_pipes)],
    },
    Help {
        id: "Pipe Riser",
        steps: &[
            "Click the corners of the shaft, then press Enter. Options: Name (the same on every floor), Lines, Height and Up or Down.",
            "In Lines, type what goes through the shaft, separated by commas: CHW-S, CHW-R, HW-S, HW-R, CO-S, CO-R, COND, REF (liquid and gas), REF-HR (three tubes), REF-L, REF-G.",
            "In Properties: the size of each line (inches for refrigerant) and what it carries to the other floors (kW, l/h for a drain, capacity index for refrigerant). Add or remove lines there, and type the height from floor to floor.",
            "End the pipes of the floor in the shaft, or start them there: they are read through it. Size Pipes sizes the lines, and the takeoff counts them by the height.",
        ],
        figures: &[("", pipe_riser)],
    },
    Help {
        id: "Name Networks",
        steps: &[
            "A network is the pipes of one service that are joined end to end, through the parts on them, the units they end in and the shaft of a riser.",
            "This tool names every network that has no name (CHW1, HW1…) and gives each its own colour, so that two networks of the same service can be told apart.",
            "Type a name on one pipe in Properties to name its whole network; there you also choose its colour and, for a print without colour, its line type.",
            "A pipe or a part drawn on a named network takes its name. The legend and the takeoff follow the names.",
        ],
        figures: &[("", name_networks)],
    },
    Help {
        id: "Network Legend",
        steps: &[
            "Click where the upper left corner of the legend goes.",
            "It has a row for each network: a sample in its colour and line type, its name, its pipes and their length.",
            "It is redrawn by itself when a network is named, renamed or restyled.",
        ],
        figures: &[("", network_legend)],
    },
    Help {
        id: "Library…",
        steps: &[
            "Click a part, then click where it goes; or drag it onto the drawing to drop it there.",
            "A valve, a filter or a pump clicked on a pipe turns along it and cuts it. For the others you are asked the orientation: a second click, an angle, or Enter.",
            "In Properties: its tag, its size (height, width, depth), volume, power, pressure loss, the outlets of a box or a header, and another model of the same kind.",
            "Keep a part of the drawing as a model of your own (My models): it is saved with the drawing.",
        ],
        figures: &[("", library)],
    },
    Help {
        id: "Check Refrigerant",
        steps: &[
            "Draw the outdoor unit, the links (one line for the liquid and the gas), the Y joints, the headers and the branch boxes, and the indoor units. The pipes must end on them.",
            "It follows the network from each outdoor unit and counts the units and the capacity index (cooling kW × 10) at each outlet.",
            "It tells what is over the limits: 5 units and index 140 per outlet of a box, and the joint or header that suits the index below it.",
            "A network can start or end at a pipe riser: what the other floors serve is typed on its line.",
        ],
        figures: &[("", check_refrigerant)],
    },
    Help {
        id: "Convert to Openings",
        steps: &[
            "It looks at the door and window blocks of a plan that lie on walls (by the words in their block or layer names).",
            "It lists what it found; accept, and each becomes an opening you can change in Properties.",
            "The old blocks are kept on a frozen layer, A-OPENING-OLD: thaw it to see them.",
        ],
        figures: &[("", convert_openings)],
    },
    Help {
        id: "Wall Elevation",
        steps: &[
            "Click the wall, then the point where the elevation is drawn, next to the plan.",
            "It shows the wall seen from the side, with its openings, sills and heights.",
            "Run it again after changing the wall or its openings.",
        ],
        figures: &[("", wall_elevation)],
    },
    Help {
        id: "Wall Junctions",
        steps: &[
            "Walls are joined where they meet (corner, tee or cross) as they are drawn, by the priority of their layers.",
            "After moving, copying or erasing walls, run this tool: it joins them again.",
            "A wall drawn first gives way to a wall of higher priority; layers of equal priority are joined without a line.",
        ],
        figures: &[("", wall_junctions)],
    },
    Help {
        id: "Corner Window",
        steps: &[
            "Click near the corner of two walls that meet.",
            "A window is put on each wall, from the corner, of the length given by the Width option (1200 mm by default).",
            "They are two ordinary openings: change each of them in Properties.",
        ],
        figures: &[("", corner_window)],
    },
    Help {
        id: "Ceiling Grid",
        steps: &[
            "Click the corners of the room, then press Enter. Or choose Walls and click inside a room closed by walls, or Room or Object.",
            "In Properties type the tile (600x600, 600x1200…) and the height of the false ceiling.",
            "The whole and cut tiles are counted in the Structure Takeoff; the tile centres are where a diffuser or a luminaire snaps.",
        ],
        figures: &[("", ceiling_grid)],
    },
    Help {
        id: "Stair",
        steps: &[
            "Click the middle of the first riser, then a point in the direction of the way up.",
            "In Properties type the rise from floor to floor, the largest riser, the tread and the width: the number of risers follows.",
            "The label says whether the stair is comfortable: twice the riser plus the tread within 600 to 650 mm (Blondel).",
        ],
        figures: &[("", stair)],
    },
    Help {
        id: "Railing",
        steps: &[
            "Click the points of the path of the railing, then press Enter.",
            "In Properties type the height and the spacing of the posts.",
            "The railings and their length are counted in the Structure Takeoff.",
        ],
        figures: &[("", railing)],
    },
    Help {
        id: "Title Block",
        steps: &[
            "Fill in the project, the parties with their logos, the picture, the revisions and the code of the sheet, then press Insert.",
            "In a layout the column goes to the right edge of the sheet; double-click it to change it.",
            "Each logo and the picture of the project are kept in the drawing; the table of revisions grows with a new line.",
        ],
        figures: &[("", title_block)],
    },
    Help {
        id: "Curtain Wall",
        steps: &[
            "Click the points of the façade, then press Enter. The Height option sets its height.",
            "In Properties type the spacing of the mullions and of the transoms, and the depth of the frame.",
            "The panels, the mullions, the transoms and the area are counted in the Structure Takeoff.",
        ],
        figures: &[("", curtain_wall)],
    },
    Help {
        id: "Building Section",
        steps: &[
            "Click the two ends of the cut line across the plan, so that it goes through the walls, slabs, columns and beams to show.",
            "Click where the section goes, or press Enter to put it to the right of the drawing.",
            "The cut walls are drawn with their openings, with the slabs, steel columns and beams and false ceilings at their elevations, and the levels in the margin.",
            "It is a picture made now: draw a new section after changing the plan. Slopes are not drawn.",
        ],
        figures: &[("", building_section)],
    },
    Help {
        id: "Roof",
        steps: &[
            "Click the corners of the roof, then press Enter. Or choose Walls and click inside a room closed by walls.",
            "Choose Shed, Gable or Hip and the pitch in degrees with the options; the ridge runs along the first side.",
            "The label gives the top of the roof. In Properties change the kind, the pitch, the direction of the ridge and the height of the eaves.",
            "The area in plan, the area of the slopes, the rise and the ridge are counted in the Structure Takeoff.",
        ],
        figures: &[("", roof)],
    },
    Help {
        id: "Column",
        steps: &[
            "Choose the section (family and size) in the list above the tools.",
            "Click where the columns stand: near the corner of two walls, the end of a beam or another column, the click lands on it.",
            "In Properties type the height of the column and the elevation of its foot.",
        ],
        figures: &[("", column)],
    },
    Help {
        id: "Beam",
        steps: &[
            "Choose the section (family and size) in the list above the tools.",
            "Click the points of the beams: one beam is drawn for each segment.",
            "A beam is cut where it meets a column or another beam. In Properties type the elevation of its underside.",
        ],
        figures: &[("", beam)],
    },
    Help {
        id: "Edit Structure",
        steps: &[
            "Select the columns or the beams, then start the tool.",
            "Choose what to change: the section, the height, the elevation or the rotation, and type the new value.",
            "The same values are changed in Properties, with the section list above the tools.",
        ],
        figures: &[("", edit_structure)],
    },
    Help {
        id: "Structure Joints",
        steps: &[
            "Beams are cut where they meet a column or another beam as they are drawn.",
            "After moving, copying or erasing columns or beams, run this tool: it cuts them again.",
            "Nothing is asked: every column and beam of the drawing is redrawn.",
        ],
        figures: &[("", structure_joints)],
    },
    Help {
        id: "Structure Takeoff",
        steps: &[
            "No selection needed: the whole drawing is counted.",
            "The steel columns and beams are listed by section with their count, length and mass.",
            "The slabs, the false ceilings, the stairs, the railings, the curtain walls and the roofs are counted too.",
            "It saves a workbook (.xlsx) or a text file.",
        ],
        figures: &[("", structure_takeoff)],
    },
    Help {
        id: "Slab Opening",
        steps: &[
            "Select the slab, then click the corners of the opening and press Enter.",
            "The area of the opening is taken out of the area and the volume of the slab.",
            "Edit Slab changes its kind, its thickness and its elevation afterwards.",
        ],
        figures: &[("", slab_opening)],
    },
    Help {
        id: "Electrical Device",
        steps: &[
            "Choose a kind of device (socket, switch, luminaire, distribution board…), then click where it goes; each click places one. At the command line you can type Name, Circuit, Power and Height first.",
            "Give each device its circuit, written board-circuit (TD1-C3): the board's name, a hyphen, the circuit's number. Devices with the same circuit name are on the same circuit.",
            "The power (W) is what the circuit carries; a luminaire's flux (lm) is what Check Lighting uses. A distribution board has a main breaker (A) and a size, and is the source of its circuits.",
            "A ceiling light can snap to the centre of the ceiling tile it is on (Ceiling Snap, see Ceiling Grid). The address of a KNX or DALI device is written beside it.",
            "Everything can be changed later in Properties: name, circuit, power, height, address. Several selected devices change together.",
        ],
        figures: &[("", elec_device)],
    },
    Help {
        id: "Cable",
        steps: &[
            "Choose the use (power, lighting, data, safety), then click the points of the run: one cable is drawn per segment, seen from above, in the colour of its use.",
            "Name its circuit (TD1-C3): the cables of one circuit share a colour, and the check, the sizing and the quantities group them by circuit.",
            "In Properties or at the command line: single or three phases, copper or aluminium, PVC or PR insulation, the laying letter (B, C, E, F), the section (empty: to be sized), the breaker (A), the power factor and the ambient temperature.",
            "The length counted for a cable is its drawing plus Extra (metres of rises and slack for the run). Use an Electrical Riser for the height between floors.",
            "Cables that carry several circuits in one bundle are marked Grouped: the current capacity is reduced as the standard says.",
        ],
        figures: &[("", cable_fig)],
    },
    Help {
        id: "Assign to Circuit",
        steps: &[
            "Select the devices, cables, HVAC equipment and air handling units that belong together, then start the tool.",
            "Type the circuit name (TD1-C3). They all take it; typing nothing puts them on no circuit.",
            "The power of an HVAC unit or equipment (its electrical kW in Properties) then counts on that circuit, and its cables are sized for it.",
            "Run Check Circuits afterwards: it says what the circuit carries and what is wrong with it.",
        ],
        figures: &[("", circuit_fig)],
    },
    Help {
        id: "Check Circuits",
        steps: &[
            "Run it when the devices, equipment and cables are drawn and each is on its circuit. No selection is needed: every circuit of the drawing is checked.",
            "For each circuit: its power (with the share on at once), the design current, the section the heating needs, the one the voltage drop needs (3 % for lighting, 5 % for the rest from the board), the section drawn and the breaker.",
            "It also gives the short-circuit current at the end of the cable (maximum and minimum), the longest cable the magnetic trip still sees, the breaking capacity against the source's current and the ratio to the board's main breaker (selectivity).",
            "Boards fed by a circuit of another board are followed: the load, the short-circuit current and the voltage drop add up down the chain.",
            "Options: the short-circuit current at the origin (kA, given by the network operator: 10 by default), the breakers' breaking capacity, the curve B, C or D and the share of power on at once. The values are design values: check them with the manufacturer.",
        ],
        figures: &[("", circuit_fig)],
    },
    Help {
        id: "Size Circuits",
        steps: &[
            "Run it after Check Circuits shows what each circuit needs. Without a selection it sizes every circuit.",
            "It writes on the cables of each circuit the section it needs and, where there is none, the breaker rating.",
            "A section you typed is kept, unless you ask to overwrite the sections already given.",
            "Run Check Circuits again: it confirms the voltage drop and the short-circuit current with the new sections.",
        ],
        figures: &[("", circuit_fig)],
    },
    Help {
        id: "Check Lighting",
        steps: &[
            "Draw the spaces (rooms) of the floor and give each its use; place the luminaires inside them and type the flux of each (lm) in Properties.",
            "The tool computes the illuminance each space gets: flux × utilisation × maintenance ÷ area (utilisation 0.5 and maintenance 0.8 by default).",
            "It compares the result with the minimum of the Code du travail (article R4223-4: 40 lux for a circulation, 120 for a room…) and lists the spaces that are too dark.",
            "It is a quick estimate, not a lighting study: the minimum values are the ones of the regulation, to be confirmed for your building.",
        ],
        figures: &[("", lighting_fig)],
    },
    Help {
        id: "Check Emergency Lighting",
        steps: &[
            "Place the emergency lights and the exit signs (Safety group) and draw the spaces, naming the escape routes (circulation, couloir, hall, escalier…) and giving each room its occupants.",
            "The check says where nothing is drawn: an escape route with no emergency light or exit sign, and a room of 100 people or more, or with more than one person for 10 m², with no emergency light.",
            "It checks the presence only. The levels and the autonomy are those of the arrêté of 14 December 2011 and of the standard NF EN 1838, to be confirmed.",
        ],
        figures: &[("", lighting_fig)],
    },
    Help {
        id: "Circuit Legend",
        steps: &[
            "Start the tool and click where the upper left corner of the legend goes.",
            "It draws one row for each circuit: a sample of its cable in its colour and line type, its name, its use, its cable, its length, its power and its breaker.",
            "It is redrawn by itself when a circuit is named, renamed, sized or changed: there is nothing to run again.",
        ],
        figures: &[("", circuit_fig)],
    },
    Help {
        id: "Single-Line Diagram",
        steps: &[
            "Start the tool and click where the upper left corner of the diagram goes; with a board selected in the options, only that board is drawn.",
            "Each board is a busbar fed by its main breaker; each circuit is a branch with its breaker and curve, its cable (3G2.5, 5G10…) and length, the minimum short-circuit current and a box with its load: devices, kW and A.",
            "A “!” beside a circuit means the check has a warning for it. Circuits with no board drawn are listed under a dash.",
            "A board fed by a circuit of another board shows “-> TD1” in the box of that circuit, and the load, the short-circuit current and the voltage drop follow the chain.",
            "It is redrawn by itself when a circuit is named, sized or changed. The options (source current, breaking capacity, curve) are those of Check Circuits.",
        ],
        figures: &[("", single_line_fig)],
    },
    Help {
        id: "Electrical Riser",
        steps: &[
            "Start the tool and click the corners of the shaft, then press Enter. The options are Name (the same on every floor), Circuit, Up or Down.",
            "A riser holds several lines, as the pipe risers do: in Properties, Add a line for each cable that goes through the shaft, and give each its use (power, lighting, data, safety, bus), its circuit, its section and its loads.",
            "Type the height from floor to floor: it is added to the cable length of the circuit of each line (voltage drop, short circuit, quantities). The load that comes from the other floors is added to the circuit's load.",
            "The arrows show the way the current goes (up or down); the end on the side of the source has the whole load of this level, the other the part that goes on to the other floors. The arrows never overlap, whatever the number of lines.",
            "Each floor is its own drawing: the risers of two floors meet by their name.",
        ],
        figures: &[("", elec_riser_fig)],
    },
    Help {
        id: "Furniture",
        steps: &[
            "Choose a piece in the list (living room, dining, bedroom, kitchen, appliances, TV and computers, desks, meeting and reception, storage), then click where the middle of its back goes, against the wall. Rotate (R) turns the next ones; Type (T) changes the model.",
            "It is drawn seen from above at its usual size. In Properties change its model, its width, its depth and its height, its altitude and, for an appliance, the power it draws; a name written beside it is optional.",
            "A piece keeps its look when it is made wider or deeper: a sofa 2.40 m long, a desk 1.80 m wide.",
            "The refrigerators, ovens, televisions, computers and the other appliances have their usual power: it is the value to put on a circuit.",
        ],
        figures: &[("", furniture_fig)],
    },
    Help {
        id: "Sanitary Fixture",
        steps: &[
            "Choose the fixture in the list, then click where the middle of its back goes, against the wall. Rotate (R) turns the next ones; Type (T) changes the kind.",
            "It is drawn seen from above at its usual size, with its drain written beside it: WC EV Ø100, LAV EU Ø40.",
            "Water closets, urinals and slop sinks drain to soil water (EV), the others to waste water (EU). In Properties change its size, its drain, the drain's diameter, its cold and hot water flows (l/s) and its altitude.",
            "The values are the usual ones in France (DTU 60.11), to be checked against the fixture chosen.",
        ],
        figures: &[("", sanitary_fig)],
    },
    Help {
        id: "Plumbing Networks",
        steps: &[
            "Choose the network, then click the points of the pipe, as for any pipe: the elbows and the reducers are drawn as soon as the pipes are laid.",
            "Waste water (EU), soil water (EV) and rain water (EP) flow by gravity: give the slope in Properties (i = 1 %, 2 %…); it is written on the pipe with its outer diameter (Ø100 EV i=2 %).",
            "Cold water (EF), domestic hot water (ECS) and its return loop are written with the outer diameter of their tube (Ø20 EF).",
            "A stack between the floors is a pipe riser: draw the shaft, then add a line for each network that goes up or down in it.",
        ],
        figures: &[("", plumbing_networks_fig)],
    },
    Help {
        id: "Connect Fixtures",
        steps: &[
            "Draw the collectors first: a waste-water pipe and a soil-water pipe along the rooms, toward the stack.",
            "Put the fixtures, then start this tool: from each fixture's drain it draws the branch to the nearest collector of its kind, within 6 m.",
            "Each branch is as wide as the fixture's drain (Ø100 for a WC, Ø40 for a basin) with a slope of 2 % for soil water and 1 % for waste water.",
            "A fixture with no collector of its kind in reach is named on the command line; run the tool again once the collector is drawn: the fixtures already connected are left.",
        ],
        figures: &[("", connect_fixtures_fig)],
    },
    Help {
        id: "Electrical Quantities",
        steps: &[
            "Draw one drawing per level, then open this tool on the level and type its name.",
            "The tables count the cables by section and use (the vertical runs of the electrical risers included), the devices by kind with their power and flux, the boards and the loads of the HVAC equipment.",
            "The circuits are listed as they are checked: power, current, sections, voltage drop, short circuit, breakers.",
            "The spreadsheet button saves them for Excel, with the prices you type; each level makes its own file.",
        ],
        figures: &[("", quantities)],
    },
    Help {
        id: "Control Device",
        steps: &[
            "Place the controllers, the modules and the gateways first and give each a name (AU1, MX1…): the other devices are wired to them by that name.",
            "Then the sensors, the room controls and the actuators. In Properties type the controller each is wired to and the equipment it serves (CTA1, UI3…).",
            "A controller or a module offers inputs and outputs (AI, AO, DI, DO); each device asks for points, a thermostat for two, a sensor for one.",
            "Give an automation device the circuit that feeds it and, if it is on a bus, the bus and its address.",
        ],
        figures: &[("", points_list)],
    },
    Help {
        id: "Bus cable",
        steps: &[
            "A bus is a name, not an object: give every device and controller on it the same Bus in Properties (BUS1).",
            "Draw its cable with this tool: the circuit of that cable has the name of the bus. Without one, Check Controls says “no bus cable is drawn”.",
            "Say the protocol on a controller or a gateway (Modbus RTU, BACnet MS/TP, KNX TP, M-Bus): the check then tests the addresses, the number of devices on a segment and the length of cable.",
            "The length counted is the cable drawn plus the height of the bus risers that carry the bus between floors.",
        ],
        figures: &[("", control_schema_fig)],
    },
    Help {
        id: "Check Controls",
        steps: &[
            "Run it when the devices and controllers are drawn. It needs no selection.",
            "It says what each device asks of its controller and the load of each controller and module against what it offers, with the reserve left.",
            "It tells what is wrong: a device wired to nothing or to a controller that is not drawn, a controller over its capacity, an automation device that no circuit feeds, a bus with no cable, a device that serves an object that is not drawn.",
            "With a protocol on the bus it also checks the addresses (Modbus 1–247, BACnet MS/TP 0–127, KNX 1.1.5, M-Bus 1–250), the devices per segment and the cable length. These are usual design values: confirm them with the manufacturer.",
        ],
        figures: &[("", points_list)],
    },
    Help {
        id: "Addresses",
        steps: &[
            "Show Addresses writes the address beside every object that has one: control devices, equipment, air handling units and electrical devices. Hide Addresses takes them off the plan.",
            "The address is a Modbus number, a BACnet MAC or a KNX address (1.1.5), typed in the Address line of the object's Properties.",
            "The list tells the addresses used twice on one bus or on one controller: those are the clashes to fix.",
        ],
        figures: &[("", points_list)],
    },
    Help {
        id: "Principle Diagram",
        steps: &[
            "Start the tool and click where the upper left corner of the diagram goes. With a bus named in the options only that bus is drawn; else all of them.",
            "It draws the buses, the controllers on each, and each controller with its modules as boxes: name, kind, and the points used and offered (AI, AO, DI, DO).",
            "Under each controller are the points wired to it, with their address and type (“@3  TA1  AI”). Controllers with no bus are drawn together.",
            "It is redrawn by itself when a control device is added, changed or erased: there is nothing to run again.",
        ],
        figures: &[("", control_schema_fig)],
    },
    Help {
        id: "Bus Riser",
        steps: &[
            "Start the tool and click the corners of the shaft, then press Enter. Name it the same on every floor and give the bus it carries (BUS1).",
            "A riser can carry several lines, a bus and the data cables for example: in Properties, Add a line for each and give its use and its circuit or bus.",
            "Type the height from floor to floor: it is added to the length of the bus, which Check Controls compares with what the protocol allows (1200 m in RS-485, 1000 m in KNX, 350 m in M-Bus).",
            "A bus riser counts as a bus cable drawn: Check Controls no longer says there is none.",
        ],
        figures: &[("", elec_riser_fig)],
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
    // A page with several drawings opens tall (it scrolls inside).
    let height = if help.figures.len() > 1 { 600.0 } else { 0.0 };
    egui::Window::new(crate::i18n::t(help.id))
        .id(egui::Id::unique("hvac_help_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_height(height)
        .show(ctx, |ui| {
            ui.set_width(440.0);
            // A page with several drawings (and captions) can be taller than the screen: it scrolls.
            let tall = help.figures.len() > 1;
            let content = |ui: &mut egui::Ui| {
                for (caption, figure) in help.figures {
                    if !caption.is_empty() {
                        ui.label(egui::RichText::new(crate::i18n::t(caption)).strong());
                    }
                    let (r, _) = ui.allocate_exact_size(vec2(440.0, if tall { 150.0 } else { 170.0 }), egui::Sense::hover());
                    let t = Tokens::get();
                    ui.painter().rect_filled(r, 4.0, t.chrome_dark);
                    figure(&ui.painter().with_clip_rect(r), r.shrink(10.0));
                    ui.add_space(6.0);
                }
                ui.add_space(2.0);
                for (i, step) in help.steps.iter().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{}.", i + 1));
                        ui.label(crate::i18n::t(step));
                    });
                }
            };
            if tall {
                egui::ScrollArea::vertical().max_height(560.0).auto_shrink([false, true]).show(ui, content);
            } else {
                content(ui);
            }
        });
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
    // Where the duct is cut: at the edges of the zone.
    for x in [z.left(), z.right()] {
        p.line_segment([pos2(x, y - 30.0), pos2(x, y + 30.0)], Stroke::new(2.0, ZONE));
    }
    text(p, pos2(r.left() + 6.0, y + 32.0), "cut at the edges", GREY);
}

/// Step 1: the outline of the zone drawn over a round duct that crosses it.
fn zone_plan(p: &Painter, r: Rect) {
    let y = r.center().y;
    let z = Rect::from_min_max(pos2(r.center().x - 70.0, r.top()), pos2(r.center().x + 70.0, r.bottom()));
    p.rect_filled(z, 0.0, ZONE.gamma_multiply(0.18));
    hatch(p, z);
    p.rect_stroke(z, 0.0, Stroke::new(1.6, ZONE), egui::StrokeKind::Inside);
    hduct(p, r.left(), r.right(), y, 24.0);
    let patch = |at: egui::Pos2, w: f32| p.rect_filled(Rect::from_min_size(at, vec2(w, 16.0)), 2.0, Tokens::get().chrome_dark);
    patch(pos2(z.left() + 4.0, r.top() + 4.0), 120.0);
    text(p, pos2(z.left() + 8.0, r.top() + 12.0), "ZONE 1  H 300 mm", ZONE);
    text(p, pos2(r.left() + 6.0, y - 20.0), "Ø400", DUCT);
    text(p, pos2(r.right() - 150.0, r.bottom() - 4.0), "seen from above", GREY);
}

/// Step 2: a section under the beam: 300 mm are left, the round Ø400 does not fit, a flat 600×200 does.
fn zone_section(p: &Painter, r: Rect) {
    let ceiling = Rect::from_min_max(pos2(r.left(), r.top()), pos2(r.right(), r.top() + 14.0));
    p.rect_filled(ceiling, 0.0, GREY.gamma_multiply(0.35));
    text(p, pos2(r.left() + 4.0, ceiling.center().y), "ceiling / slab", GREY);
    let beam = Rect::from_min_max(pos2(r.center().x - 80.0, ceiling.bottom()), pos2(r.center().x + 80.0, ceiling.bottom() + 34.0));
    p.rect_filled(beam, 0.0, ZONE.gamma_multiply(0.18));
    p.rect_stroke(beam, 0.0, Stroke::new(1.4, ZONE), egui::StrokeKind::Inside);
    text(p, beam.left_top() + vec2(6.0, 10.0), "beam", ZONE);
    let low = beam.bottom();
    p.line_segment([pos2(beam.left() - 6.0, low), pos2(beam.right() + 6.0, low)], Stroke::new(1.0, ZONE));
    // The round duct outside, too tall for what is under the beam; the flat one inside.
    let cy = low + 20.0;
    p.circle_filled(pos2(r.left() + 50.0, cy), 28.0, DUCT_FILL);
    p.circle_stroke(pos2(r.left() + 50.0, cy), 28.0, stroke());
    text(p, pos2(r.left() + 30.0, cy), "Ø400", DUCT);
    p.circle_filled(pos2(r.right() - 50.0, cy), 28.0, DUCT_FILL);
    p.circle_stroke(pos2(r.right() - 50.0, cy), 28.0, stroke());
    text(p, pos2(r.right() - 70.0, cy), "Ø400", DUCT);
    let flat = Rect::from_center_size(pos2(r.center().x, low + 14.0), vec2(70.0, 22.0));
    p.rect_filled(flat, 0.0, DUCT_FILL);
    p.rect_stroke(flat, 0.0, stroke(), egui::StrokeKind::Inside);
    text(p, flat.left_top() + vec2(8.0, 11.0), "600×200", DUCT);
    text(p, pos2(beam.left() + 6.0, beam.bottom() + 40.0), "300 mm under the beam", ZONE);
    text(p, pos2(r.left() + 26.0, r.bottom() - 4.0), "outside", GREY);
    text(p, pos2(r.center().x - 24.0, r.bottom() - 4.0), "in the zone", GREY);
}

/// Step 4: a section with an altitude: the duct goes down by two 45° elbows into the zone and up again.
fn zone_altitude(p: &Painter, r: Rect) {
    let (hi, lo) = (r.top() + 40.0, r.top() + 78.0);
    for (y, label) in [(hi, "3.00 m"), (lo, "2.60 m")] {
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(0.8, GREY.gamma_multiply(0.6)));
        text(p, pos2(r.left() + 2.0, y - 8.0), label, GREY);
    }
    let zone_box = Rect::from_min_max(pos2(r.center().x - 60.0, r.top() + 6.0), pos2(r.center().x + 60.0, hi));
    p.rect_filled(zone_box, 0.0, ZONE.gamma_multiply(0.18));
    p.rect_stroke(zone_box, 0.0, Stroke::new(1.2, ZONE), egui::StrokeKind::Inside);
    text(p, zone_box.left_top() + vec2(6.0, 12.0), "zone: altitude 2.60 m", ZONE);
    let (x0, x1, x2, x3) = (r.left() + 10.0, r.center().x - 60.0, r.center().x + 60.0, r.right() - 10.0);
    let path = [pos2(x0, hi), pos2(x1 - 26.0, hi), pos2(x1, lo), pos2(x2, lo), pos2(x2 + 26.0, hi), pos2(x3, hi)];
    p.add(egui::Shape::line(path.to_vec(), Stroke::new(8.0, DUCT_FILL)));
    p.add(egui::Shape::line(path.to_vec(), Stroke::new(1.5, DUCT)));
    text(p, pos2(x0 + 46.0, hi - 12.0), "Al: 3.00", DUCT);
    text(p, pos2(r.center().x - 24.0, lo + 18.0), "Al: 2.60", DUCT);
    text(p, pos2(x3 - 70.0, hi - 12.0), "Al: 3.00", DUCT);
    text(p, pos2(x1 - 70.0, lo + 4.0), "2 × 45°", GREY);
    text(p, pos2(x2 + 4.0, lo + 4.0), "2 × 45°", GREY);
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
        assert_eq!(HELPS.len(), 54);
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
            let mut h = Harness::builder().with_size(vec2(480.0, 800.0)).build_ui(|ui| {
                show(ui.ctx());
            });
            request(&h.ctx, help.id);
            h.run_steps(8);
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

fn title_block(p: &Painter, r: Rect) {
    let at = fx(r);
    let (a, b) = (at(0.3, 0.05), at(0.7, 0.95));
    p.rect_stroke(Rect::from_two_pos(a, b), 0.0, Stroke::new(1.5, GREY), egui::StrokeKind::Inside);
    for y in [0.2, 0.4, 0.78, 0.88] {
        p.line_segment([at(0.3, y), at(0.7, y)], Stroke::new(1.0, GREY));
    }
    p.rect_filled(Rect::from_two_pos(at(0.34, 0.46), at(0.66, 0.7)), 0.0, ZONE);
    text(p, at(0.32, 0.12), "project", GREY);
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

const CABLE_RED: Color32 = Color32::from_rgb(0xd0, 0x45, 0x45);
const LIGHT_YELLOW: Color32 = Color32::from_rgb(0xe6, 0xc2, 0x2e);
const BUS_PURPLE: Color32 = Color32::from_rgb(0x90, 0x60, 0xd0);
/// The waste water and the soil water of the plumbing figures.
const GREEN_EU: Color32 = Color32::from_rgb(0x3f, 0xb5, 0x6a);
const BROWN_EV: Color32 = Color32::from_rgb(0xa8, 0x6a, 0x38);

/// A point of the figure's box from fractions of its width and height.
fn frac(r: Rect) -> impl Fn(f32, f32) -> egui::Pos2 {
    move |x, y| pos2(r.left() + x * r.width(), r.top() + y * r.height())
}

fn board(p: &Painter, c: egui::Pos2, label: &str) {
    let r = Rect::from_center_size(c, vec2(44.0, 56.0));
    p.rect_filled(r, 0.0, Color32::from_gray(40));
    p.rect_stroke(r, 0.0, Stroke::new(1.4, Color32::WHITE), egui::StrokeKind::Inside);
    p.text(c, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), Color32::WHITE);
}

fn socket(p: &Painter, c: egui::Pos2) {
    p.circle_stroke(c, 7.0, Stroke::new(1.3, Color32::WHITE));
    p.line_segment([c + vec2(-3.0, -3.0), c + vec2(-3.0, 3.0)], Stroke::new(1.3, Color32::WHITE));
    p.line_segment([c + vec2(3.0, -3.0), c + vec2(3.0, 3.0)], Stroke::new(1.3, Color32::WHITE));
}

fn luminaire(p: &Painter, c: egui::Pos2) {
    p.circle_stroke(c, 8.0, Stroke::new(1.3, LIGHT_YELLOW));
    p.line_segment([c + vec2(-5.0, -5.0), c + vec2(5.0, 5.0)], Stroke::new(1.0, LIGHT_YELLOW));
    p.line_segment([c + vec2(-5.0, 5.0), c + vec2(5.0, -5.0)], Stroke::new(1.0, LIGHT_YELLOW));
}

fn elec_device(p: &Painter, r: Rect) {
    let at = frac(r);
    board(p, at(0.12, 0.5), "TD1");
    for (y, color) in [(0.2, CABLE_RED), (0.5, CABLE_RED), (0.8, LIGHT_YELLOW)] {
        p.line_segment([at(0.2, 0.5), at(0.4, y)], Stroke::new(1.3, color));
        p.line_segment([at(0.4, y), at(0.62, y)], Stroke::new(1.3, color));
    }
    socket(p, at(0.66, 0.2));
    socket(p, at(0.66, 0.5));
    luminaire(p, at(0.66, 0.8));
    text(p, at(0.74, 0.2), "TD1-C1  P1", CABLE_RED);
    text(p, at(0.74, 0.5), "TD1-C1  P2", CABLE_RED);
    text(p, at(0.74, 0.8), "TD1-E1  L1", LIGHT_YELLOW);
}

fn cable_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    for (y, color, label) in
        [(0.2, CABLE_RED, "TD1-C1  3G2.5"), (0.5, LIGHT_YELLOW, "TD1-E1  3G1.5"), (0.8, Color32::from_rgb(0x3c, 0xb0, 0x6b), "VDI  Cat6")]
    {
        p.line_segment([at(0.05, y), at(0.5, y)], Stroke::new(1.6, color));
        p.line_segment([at(0.5, y), at(0.62, y - 0.12)], Stroke::new(1.6, color));
        p.line_segment([at(0.62, y - 0.12), at(0.95, y - 0.12)], Stroke::new(1.6, color));
        text(p, at(0.07, y - 0.1), label, color);
    }
    text(p, at(0.5, 0.97), "+ Extra (rises, slack)", GREY);
}

fn circuit_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    board(p, at(0.1, 0.5), "TD1");
    for (y, color, name) in [(0.25, CABLE_RED, "TD1-C1"), (0.75, LIGHT_YELLOW, "TD1-E1")] {
        p.line_segment([at(0.18, 0.5), at(0.3, y)], Stroke::new(1.5, color));
        p.line_segment([at(0.3, y), at(0.85, y)], Stroke::new(1.5, color));
        text(p, at(0.32, y - 0.1), name, color);
        for x in [0.5, 0.65, 0.8] {
            if color == CABLE_RED { socket(p, at(x, y)) } else { luminaire(p, at(x, y)) }
        }
    }
    text(p, at(0.32, 0.5), "230 V  12 A  16 mm²  ΔU 2.1 %", GREY);
}

fn lighting_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    let room = Rect::from_min_max(at(0.1, 0.12), at(0.65, 0.88));
    p.rect_stroke(room, 0.0, Stroke::new(1.6, WALL), egui::StrokeKind::Inside);
    for (x, y) in [(0.25, 0.3), (0.5, 0.3), (0.25, 0.68), (0.5, 0.68)] {
        luminaire(p, at(x, y));
    }
    text(p, at(0.7, 0.3), "Bureau 1", Color32::WHITE);
    text(p, at(0.7, 0.45), "4 × 3000 lm", LIGHT_YELLOW);
    text(p, at(0.7, 0.6), "≈ 320 lx  ≥ 120 lx", Color32::from_rgb(0x3c, 0xb0, 0x6b));
    p.circle_stroke(at(0.82, 0.82), 8.0, Stroke::new(1.3, Color32::from_rgb(0xd0, 0x45, 0x45)));
    text(p, at(0.86, 0.82), "SSI", Color32::from_rgb(0xd0, 0x45, 0x45));
}

fn single_line_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    p.line_segment([at(0.5, 0.05), at(0.5, 0.22)], Stroke::new(1.6, Color32::WHITE));
    p.rect_stroke(Rect::from_center_size(at(0.5, 0.15), vec2(10.0, 14.0)), 0.0, Stroke::new(1.3, Color32::WHITE), egui::StrokeKind::Inside);
    text(p, at(0.54, 0.12), "TD1  63 A", Color32::WHITE);
    p.line_segment([at(0.1, 0.25), at(0.9, 0.25)], Stroke::new(3.0, Color32::WHITE));
    for (x, color, name, load) in
        [(0.2, CABLE_RED, "C1  C16", "3.2 kW 14 A"), (0.5, LIGHT_YELLOW, "E1  B10", "1.1 kW 5 A"), (0.8, CABLE_RED, "C2  C20 !", "4.6 kW 20 A")]
    {
        p.line_segment([at(x, 0.25), at(x, 0.4)], Stroke::new(1.4, color));
        p.rect_stroke(Rect::from_center_size(at(x, 0.45), vec2(10.0, 14.0)), 0.0, Stroke::new(1.3, color), egui::StrokeKind::Inside);
        p.line_segment([at(x, 0.5), at(x, 0.7)], Stroke::new(1.4, color));
        p.text(at(x + 0.03, 0.45), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(10.0), color);
        let b = Rect::from_center_size(at(x, 0.8), vec2(74.0, 22.0));
        p.rect_stroke(b, 2.0, Stroke::new(1.2, GREY), egui::StrokeKind::Inside);
        p.text(b.center(), egui::Align2::CENTER_CENTER, load, egui::FontId::proportional(10.0), GREY);
    }
}

/// A sofa, a coffee table and a television unit, seen from above.
fn furniture_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    let s = Stroke::new(1.3, Color32::WHITE);
    p.line_segment([at(0.05, 0.1), at(0.95, 0.1)], Stroke::new(3.0, GREY));
    // The sofa against the wall: back, arms and two cushions.
    p.rect_stroke(Rect::from_min_max(at(0.1, 0.1), at(0.5, 0.42)), 4.0, s, egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(at(0.1, 0.1), at(0.5, 0.17)), 0.0, s, egui::StrokeKind::Inside);
    p.line_segment([at(0.3, 0.17), at(0.3, 0.42)], s);
    // The coffee table, and the television unit on the opposite wall.
    p.rect_stroke(Rect::from_min_max(at(0.18, 0.56), at(0.42, 0.7)), 3.0, s, egui::StrokeKind::Inside);
    p.line_segment([at(0.05, 0.92), at(0.95, 0.92)], Stroke::new(3.0, GREY));
    p.rect_stroke(Rect::from_min_max(at(0.14, 0.8), at(0.46, 0.92)), 0.0, s, egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(at(0.2, 0.84), at(0.4, 0.86)), 0.0, s, egui::StrokeKind::Inside);
    // A refrigerator and an oven beside them, with the text of their power.
    p.rect_stroke(Rect::from_min_max(at(0.62, 0.1), at(0.74, 0.3)), 0.0, s, egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(at(0.78, 0.1), at(0.9, 0.3)), 0.0, s, egui::StrokeKind::Inside);
    p.circle_stroke(at(0.84, 0.2), 0.03 * r.width(), s);
    text(p, at(0.6, 0.36), "120 W", GREY);
    text(p, at(0.77, 0.36), "3000 W", GREY);
}

/// A water closet and a basin against a wall, seen from above, with the text of their drains.
fn sanitary_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    let s = Stroke::new(1.3, Color32::WHITE);
    p.line_segment([at(0.05, 0.12), at(0.95, 0.12)], Stroke::new(3.0, GREY));
    let back = Rect::from_min_max(at(0.14, 0.12), at(0.3, 0.24));
    p.rect_stroke(back, 0.0, s, egui::StrokeKind::Inside);
    let bowl = |c: egui::Pos2, rx: f32, ry: f32| {
        let pts: Vec<egui::Pos2> = (0..32)
            .map(|k| c + vec2(rx * (k as f32 / 32.0 * std::f32::consts::TAU).cos(), ry * (k as f32 / 32.0 * std::f32::consts::TAU).sin()))
            .collect();
        p.add(egui::Shape::closed_line(pts, s));
    };
    bowl(at(0.22, 0.48), 0.07 * r.width(), 0.22 * r.height());
    let basin = Rect::from_min_max(at(0.55, 0.12), at(0.8, 0.5));
    p.rect_stroke(basin, 2.0, s, egui::StrokeKind::Inside);
    bowl(at(0.675, 0.33), 0.08 * r.width(), 0.1 * r.height());
    text(p, at(0.12, 0.8), "WC EV \u{d8}100", BROWN_EV);
    text(p, at(0.55, 0.6), "LAV EU \u{d8}40", GREEN_EU);
}

/// The plumbing networks as they are drawn: their colours and texts.
fn plumbing_networks_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    for (i, (colour, label, width)) in [
        (GREEN_EU, "\u{d8}50 EU i=1 %", 3.0),
        (BROWN_EV, "\u{d8}100 EV i=2 %", 5.0),
        (Color32::from_rgb(0x39, 0xc2, 0xb0), "\u{d8}100 EP i=1 %", 5.0),
        (Color32::from_rgb(0x2f, 0x8c, 0xff), "\u{d8}20 EF", 1.6),
        (Color32::from_rgb(0xe0, 0x24, 0x24), "\u{d8}20 ECS", 1.6),
        (Color32::from_rgb(0xff, 0x9a, 0x3c), "\u{d8}16 BECS", 1.4),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 0.1 + 0.15 * i as f32;
        p.line_segment([at(0.05, y), at(0.5, y)], Stroke::new(width, colour));
        text(p, at(0.55, y - 0.05), label, colour);
    }
}

/// Two fixtures and the branches the tool draws to their collectors.
fn connect_fixtures_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    let s = Stroke::new(1.3, Color32::WHITE);
    p.rect_stroke(Rect::from_min_max(at(0.15, 0.05), at(0.35, 0.3)), 2.0, s, egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_max(at(0.6, 0.05), at(0.72, 0.3)), 0.0, s, egui::StrokeKind::Inside);
    p.line_segment([at(0.05, 0.62), at(0.95, 0.62)], Stroke::new(3.0, GREEN_EU));
    p.line_segment([at(0.05, 0.85), at(0.95, 0.85)], Stroke::new(5.0, BROWN_EV));
    p.line_segment([at(0.25, 0.22), at(0.25, 0.62)], Stroke::new(2.2, GREEN_EU));
    p.line_segment([at(0.66, 0.2), at(0.66, 0.85)], Stroke::new(4.0, BROWN_EV));
    text(p, at(0.27, 0.36), "EU \u{d8}40 i=1 %", GREEN_EU);
    text(p, at(0.68, 0.42), "EV \u{d8}100 i=2 %", BROWN_EV);
}

fn elec_riser_fig(p: &Painter, r: Rect) {
    // A shaft with three lines; from each symbol an arrow at 45°, parallel and apart, with its text at the end: nothing overlaps.
    let shaft = Rect::from_min_size(pos2(r.right() - 150.0, r.top() + 10.0), vec2(110.0, 44.0));
    p.rect_stroke(shaft, 0.0, Stroke::new(1.4, Color32::WHITE), egui::StrokeKind::Inside);
    for (i, (color, label)) in
        [(CABLE_RED, "TD1-C1 35 mm² 18 kW"), (LIGHT_YELLOW, "TD1-E1 6 mm² 2.5 kW"), (BUS_PURPLE, "BUS1 2 mm²")].into_iter().enumerate()
    {
        let centre = pos2(shaft.left() + 24.0 + 31.0 * i as f32, shaft.center().y);
        p.circle_stroke(centre, 6.0, Stroke::new(1.3, color));
        p.circle_filled(centre, 1.5, color);
        let len = 36.0 + 18.0 * i as f32;
        let end = centre + vec2(-len, len);
        p.line_segment([centre + vec2(-4.0, 4.0), end], Stroke::new(1.2, color));
        p.arrow(end + vec2(9.0, -9.0), vec2(-9.0, 9.0), Stroke::new(1.5, color));
        p.text(end + vec2(-3.0, 2.0), egui::Align2::RIGHT_TOP, label, egui::FontId::proportional(10.0), color);
    }
}

fn control_schema_fig(p: &Painter, r: Rect) {
    let at = frac(r);
    p.line_segment([at(0.04, 0.12), at(0.96, 0.12)], Stroke::new(2.2, BUS_PURPLE));
    text(p, at(0.05, 0.04), "BUS1  Modbus RTU", BUS_PURPLE);
    for (x, name) in [(0.15, "AU1"), (0.58, "AU2")] {
        p.line_segment([at(x + 0.12, 0.12), at(x + 0.12, 0.22)], Stroke::new(1.4, BUS_PURPLE));
        let b = Rect::from_min_size(at(x, 0.22), vec2(0.3 * r.width(), 0.26 * r.height()));
        p.rect_stroke(b, 2.0, Stroke::new(1.3, Color32::WHITE), egui::StrokeKind::Inside);
        p.text(b.center(), egui::Align2::CENTER_CENTER, format!("{name}  AI 4/8  DO 2/4"), egui::FontId::proportional(10.0), Color32::WHITE);
        for (i, row) in ["@3  TA1  AI", "@4  CO2  AI"].into_iter().enumerate() {
            text(p, at(x + 0.02, 0.6 + 0.14 * i as f32), row, GREY);
        }
    }
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
            assert!((3..=8).contains(&h.steps.len()), "{}: {} steps", h.id, h.steps.len());
            for step in h.steps {
                assert_ne!(crate::i18n::tr("fr", step), *step, "{}: not translated into French: {step}", h.id);
                assert_ne!(crate::i18n::tr("zh", step), *step, "{}: not translated into Chinese: {step}", h.id);
            }
            for (caption, _) in h.figures.iter().filter(|(c, _)| !c.is_empty()) {
                assert_ne!(crate::i18n::tr("fr", caption), *caption, "{}: caption not translated into French: {caption}", h.id);
                assert_ne!(crate::i18n::tr("zh", caption), *caption, "{}: caption not translated into Chinese: {caption}", h.id);
            }
        }
        // Every figure draws without panicking, in a box of the size the window gives it.
        let mut hn = egui_kittest::Harness::new_ui(|ui| {
            for h in HELPS {
                for (_, figure) in h.figures {
                    let (r, _) = ui.allocate_exact_size(vec2(440.0, 170.0), egui::Sense::hover());
                    figure(&ui.painter().with_clip_rect(r), r.shrink(10.0));
                }
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
            "Title Block",
            "Curtain Wall",
            "Roof",
            "Building Section",
            "Column",
            "Beam",
            "Edit Structure",
            "Structure Joints",
            "Structure Takeoff",
            "Slab Opening",
            "Height Zone",
            "Electrical Device",
            "Cable",
            "Assign to Circuit",
            "Check Circuits",
            "Size Circuits",
            "Check Lighting",
            "Check Emergency Lighting",
            "Circuit Legend",
            "Single-Line Diagram",
            "Electrical Riser",
            "Electrical Quantities",
            "Control Device",
            "Bus cable",
            "Check Controls",
            "Addresses",
            "Principle Diagram",
            "Bus Riser",
        ] {
            assert!(have(id), "{id}");
        }
    }
}
