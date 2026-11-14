//! The library of pipework parts: what a designer reaches for most often on a hydraulic network (valves, filters, vents,
//! pumps, vessels, tanks, boilers, exchangers, instruments) and, for refrigerant networks, the branches, headers and
//! branch-selector boxes of Daikin's VRV ranges. The sizes and the number of outlets of the Daikin parts come from the
//! manufacturer's installation manuals (the refnet branches, 4PW29723 and 4PW23719; the VRV IV branch selectors, BS-Q14AV1B; the
//! VRV 5 branch selectors, 4P797566-1A); the symbols and the plan sizes are ours. Plan sizes are indicative: the unit's size
//! on the data sheet is typed over them.

use super::{HvacUnits, PartCategory, PartKind, PartParams};

/// A model of the library: the values a part takes when it is chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct PartModel {
    /// The English name, also the key of its translation.
    pub name: &'static str,
    pub name_fr: &'static str,
    pub category: PartCategory,
    pub kind: PartKind,
    pub brand: &'static str,
    /// The size of the symbol in plan, mm.
    pub length_mm: f64,
    pub width_mm: f64,
    pub outlets: u32,
    /// The connections and the limits, as free text.
    pub connections: String,
    pub dn_mm: Option<f64>,
    pub volume_l: Option<f64>,
    pub power_kw: Option<f64>,
}

impl PartModel {
    fn new(name: &'static str, name_fr: &'static str, category: PartCategory, kind: PartKind) -> PartModel {
        let (length_mm, width_mm) = kind.default_size_mm();
        PartModel {
            name,
            name_fr,
            category,
            kind,
            brand: "",
            length_mm,
            width_mm,
            outlets: 0,
            connections: String::new(),
            dn_mm: None,
            volume_l: None,
            power_kw: None,
        }
    }

    fn dn(mut self, dn: f64) -> PartModel {
        self.dn_mm = Some(dn);
        self
    }

    fn volume(mut self, litres: f64) -> PartModel {
        self.volume_l = Some(litres);
        self
    }

    fn power(mut self, kw: f64) -> PartModel {
        self.power_kw = Some(kw);
        self
    }

    fn outlets(mut self, n: u32) -> PartModel {
        self.outlets = n;
        self
    }

    fn size(mut self, length_mm: f64, width_mm: f64) -> PartModel {
        (self.length_mm, self.width_mm) = (length_mm, width_mm);
        self
    }

    /// A Daikin part: the brand, the number of outlets and the connections.
    fn daikin(mut self, outlets: u32, connections: &str) -> PartModel {
        self.brand = "Daikin";
        self.outlets = outlets;
        self.connections = connections.to_string();
        self
    }

    /// The part a new object of the model starts as, named `name`.
    pub fn params(&self, name: &str, units: HvacUnits) -> PartParams {
        PartParams {
            name: name.to_string(),
            kind: self.kind,
            model: self.name.to_string(),
            brand: self.brand.to_string(),
            length_mm: self.length_mm,
            width_mm: self.width_mm,
            outlets: self.outlets,
            connections: self.connections.clone(),
            dn_mm: self.dn_mm,
            volume_l: self.volume_l,
            power_kw: self.power_kw,
            flow_m3h: None,
            head_kpa: None,
            weight_kg: None,
            label_height_mm: 100.0,
            units,
        }
    }

    /// The tag a part of the model starts with ("V" for a valve, "P" for a pump).
    pub fn tag_prefix(&self) -> &'static str {
        match self.category {
            PartCategory::Valves => "V",
            PartCategory::Fittings => "F",
            PartCategory::AirAndDirt => "A",
            PartCategory::Instruments => "I",
            PartCategory::Pumps => "P",
            PartCategory::VesselsAndTanks => "R",
            PartCategory::HeatSources => "H",
            PartCategory::RefrigerantBranches => "RB",
            PartCategory::BranchBoxes => "BS",
        }
    }
}

/// The part of the library called `name`, in either language.
pub fn standard_part_model_named(name: &str) -> Option<PartModel> {
    let n = name.trim();
    standard_part_models().into_iter().find(|m| m.name.eq_ignore_ascii_case(n) || m.name_fr.eq_ignore_ascii_case(n))
}

/// Every model of the library.
pub fn standard_part_models() -> Vec<PartModel> {
    use PartCategory as C;
    use PartKind as K;
    let m = PartModel::new;
    let mut v = vec![
        // Valves.
        m("Isolation valve (ball)", "Vanne d'isolement (à boisseau sphérique)", C::Valves, K::IsolationValve).dn(50.0),
        m("Butterfly valve", "Vanne papillon", C::Valves, K::ButterflyValve).dn(100.0),
        m("Check valve", "Clapet anti-retour", C::Valves, K::CheckValve).dn(50.0),
        m("Balancing valve (TA)", "Vanne d'équilibrage manuelle (type TA)", C::Valves, K::BalancingValve).dn(50.0),
        m("Pressure independent control valve (PICV)", "Vanne de régulation indépendante de la pression (PICV)", C::Valves, K::PicValve).dn(25.0),
        m("Differential pressure controller", "Régulateur de pression différentielle", C::Valves, K::PicValve).dn(40.0),
        m("2-way control valve", "Vanne de régulation 2 voies", C::Valves, K::ControlValve).dn(32.0),
        m("3-way control valve", "Vanne de régulation 3 voies", C::Valves, K::ThreeWayValve).dn(32.0),
        m("Solenoid valve", "Électrovanne", C::Valves, K::ControlValve).dn(15.0),
        m("Safety valve", "Soupape de sécurité", C::Valves, K::SafetyValve).dn(25.0),
        m("Pressure reducing valve", "Réducteur de pression", C::Valves, K::PressureReducingValve).dn(25.0),
        m("Drain valve", "Robinet de vidange", C::Valves, K::DrainValve).dn(15.0),
        m("Gate valve", "Vanne à opercule", C::Valves, K::IsolationValve).dn(65.0),
        m("Thermostatic radiator valve", "Robinet thermostatique", C::Valves, K::ControlValve).dn(15.0),
        m("Flow limiter (dynamic balancing)", "Limiteur de débit (équilibrage dynamique)", C::Valves, K::PicValve).dn(32.0),
        m("6-way change-over valve", "Vanne 6 voies (change-over)", C::Valves, K::ThreeWayValve).dn(25.0),
        m("Motorised ball valve", "Vanne à boisseau motorisée", C::Valves, K::ControlValve).dn(40.0),
        // Fittings.
        m("Y strainer", "Filtre à tamis (type Y)", C::Fittings, K::Strainer).dn(50.0),
        m("Flexible connector", "Manchette souple (compensateur)", C::Fittings, K::FlexConnector).dn(50.0),
        m("Reducer", "Réduction", C::Fittings, K::Reducer).dn(50.0),
        m("Backflow disconnector (BA)", "Disconnecteur (type BA)", C::Fittings, K::Disconnector).dn(25.0),
        m("Filling group", "Groupe de remplissage", C::Fittings, K::FillingGroup).dn(15.0),
        m("Water meter", "Compteur d'eau", C::Fittings, K::FlowMeter).dn(25.0),
        m("Hydraulic manifold, 4 circuits", "Collecteur hydraulique, 4 départs", C::Fittings, K::Manifold).dn(65.0).outlets(4),
        m("Hydraulic manifold, 6 circuits", "Collecteur hydraulique, 6 départs", C::Fittings, K::Manifold).dn(65.0).outlets(6),
        m("Hydraulic manifold, 8 circuits", "Collecteur hydraulique, 8 départs", C::Fittings, K::Manifold).dn(80.0).outlets(8).size(1100.0, 150.0),
        // Air and dirt.
        m("Automatic air vent", "Purgeur d'air automatique", C::AirAndDirt, K::AirVent).dn(15.0),
        m("Manual air vent", "Purgeur d'air manuel", C::AirAndDirt, K::AirVent).dn(15.0),
        m("Air separator (deaerator)", "Séparateur d'air (dégazeur)", C::AirAndDirt, K::Deaerator).dn(80.0),
        m("Dirt separator", "Pot à boues (séparateur de boues)", C::AirAndDirt, K::DirtSeparator).dn(80.0),
        m("Magnetic dirt separator", "Filtre magnétique (pot à boues magnétique)", C::AirAndDirt, K::DirtSeparator).dn(50.0),
        // Instruments.
        m("Thermometer", "Thermomètre", C::Instruments, K::Thermometer),
        m("Pressure gauge", "Manomètre", C::Instruments, K::PressureGauge),
        m("Flow meter", "Débitmètre", C::Instruments, K::FlowMeter).dn(50.0),
        m("Energy meter (heat / cooling)", "Compteur d'énergie (calories / frigories)", C::Instruments, K::EnergyMeter).dn(50.0),
        m("Temperature sensor", "Sonde de température", C::Instruments, K::Sensor),
        m("Differential pressure sensor", "Capteur de pression différentielle", C::Instruments, K::Sensor),
        m("Flow switch", "Contrôleur de débit", C::Instruments, K::Sensor),
        m("Pressure switch", "Pressostat", C::Instruments, K::Sensor),
        m("Thermowell", "Doigt de gant", C::Instruments, K::Sensor),
        // Pumps.
        m("Circulator (single)", "Circulateur simple", C::Pumps, K::Pump).dn(50.0),
        m("Circulator (double)", "Circulateur double", C::Pumps, K::DoublePump).dn(50.0),
        m("Variable-speed pump", "Pompe à vitesse variable", C::Pumps, K::Pump).dn(65.0),
        m("Base-mounted pump", "Pompe sur socle", C::Pumps, K::Pump).dn(80.0).size(450.0, 300.0),
        // Vessels and tanks.
        m("Expansion vessel 24 l", "Vase d'expansion 24 l", C::VesselsAndTanks, K::ExpansionVessel).volume(24.0).size(300.0, 300.0),
        m("Expansion vessel 80 l", "Vase d'expansion 80 l", C::VesselsAndTanks, K::ExpansionVessel).volume(80.0).size(450.0, 450.0),
        m("Expansion vessel 300 l", "Vase d'expansion 300 l", C::VesselsAndTanks, K::ExpansionVessel).volume(300.0).size(650.0, 650.0),
        m("Buffer tank 300 l", "Ballon tampon 300 l", C::VesselsAndTanks, K::Tank).volume(300.0).size(600.0, 600.0),
        m("Buffer tank 500 l", "Ballon tampon 500 l", C::VesselsAndTanks, K::Tank).volume(500.0).size(750.0, 750.0),
        m("Buffer tank 1000 l", "Ballon tampon 1000 l", C::VesselsAndTanks, K::Tank).volume(1000.0).size(950.0, 950.0),
        m("Decoupling bottle", "Bouteille de découplage", C::VesselsAndTanks, K::DecouplingBottle).dn(80.0),
        m("Domestic hot water cylinder 300 l", "Ballon d'eau chaude sanitaire 300 l", C::VesselsAndTanks, K::Tank).volume(300.0).size(650.0, 650.0),
        m("Water softener", "Adoucisseur", C::VesselsAndTanks, K::Tank).size(400.0, 400.0),
        m("Glycol filling unit", "Station de remplissage glycol", C::VesselsAndTanks, K::Tank).size(500.0, 500.0),
        m("Pressurisation unit", "Groupe de maintien de pression", C::VesselsAndTanks, K::ExpansionVessel).size(500.0, 500.0),
        // Heat sources.
        m("Wall-hung gas boiler", "Chaudière gaz murale", C::HeatSources, K::Boiler).power(24.0).size(450.0, 350.0),
        m("Floor-standing condensing boiler", "Chaudière à condensation au sol", C::HeatSources, K::Boiler).power(100.0).size(900.0, 600.0),
        m("Electric boiler", "Chaudière électrique", C::HeatSources, K::Boiler).power(18.0).size(450.0, 350.0),
        m("Plate heat exchanger", "Échangeur à plaques", C::HeatSources, K::HeatExchanger).power(100.0),
        m("Wood pellet boiler", "Chaudière à granulés", C::HeatSources, K::Boiler).power(30.0).size(900.0, 700.0),
        m("Dry cooler", "Aéroréfrigérant", C::HeatSources, K::HeatExchanger).power(150.0).size(2000.0, 1000.0),
    ];
    v.extend(refrigerant_models());
    v
}

/// The refnet joints and headers and the branch-selector boxes of Daikin's VRV ranges.
fn refrigerant_models() -> Vec<PartModel> {
    use PartCategory as C;
    use PartKind as K;
    let m = PartModel::new;
    let steps = "sizes by cutting the stepped sockets (Daikin refnet installation manual)";
    let mut v = Vec::new();
    // Refnet joints (Y): the model follows the capacity index of the units downstream. Selection as in the VRV IV
    // and VRV 5 selection guides.
    for (name, name_fr, limit) in [
        ("M20T", "M20T", "capacity index below 200"),
        ("M29T", "M29T", "capacity index 200 to below 290"),
        ("M64T", "M64T", "capacity index 290 to below 640"),
        ("M75T", "M75T", "capacity index 640 or more"),
    ] {
        for (family, gas) in [("KHRQ22", "R410A"), ("KHRQ23", "R32")] {
            let (n, f): (&'static str, &'static str) = match (family, name) {
                ("KHRQ22", "M20T") => ("Refnet joint KHRQ22M20T", "Refnet Y KHRQ22M20T"),
                ("KHRQ22", "M29T") => ("Refnet joint KHRQ22M29T", "Refnet Y KHRQ22M29T"),
                ("KHRQ22", "M64T") => ("Refnet joint KHRQ22M64T", "Refnet Y KHRQ22M64T"),
                ("KHRQ22", _) => ("Refnet joint KHRQ22M75T", "Refnet Y KHRQ22M75T"),
                (_, "M20T") => ("Refnet joint KHRQ23M20T", "Refnet Y KHRQ23M20T"),
                (_, "M29T") => ("Refnet joint KHRQ23M29T", "Refnet Y KHRQ23M29T"),
                (_, "M64T") => ("Refnet joint KHRQ23M64T", "Refnet Y KHRQ23M64T"),
                _ => ("Refnet joint KHRQ23M75T", "Refnet Y KHRQ23M75T"),
            };
            let _ = (name_fr, gas);
            v.push(m(n, f, C::RefrigerantBranches, K::RefnetJoint).daikin(2, &format!("{gas}: {limit}; {steps}")));
        }
    }
    // Refnet headers: up to 8 branches, the model following the capacity index below.
    for (n, f, gas, limit) in [
        ("Refnet header KHRQ22M29H", "Nourrice Refnet KHRQ22M29H", "R410A", "capacity index below 290"),
        ("Refnet header KHRQ22M64H", "Nourrice Refnet KHRQ22M64H", "R410A", "capacity index 290 to below 640"),
        (
            "Refnet header KHRQ22M75H",
            "Nourrice Refnet KHRQ22M75H",
            "R410A",
            "capacity index 640 or more (needed when the pipe above is 1 3/8\" or larger)",
        ),
        ("Refnet header KHRQ23M29H", "Nourrice Refnet KHRQ23M29H", "R32", "capacity index below 290"),
        ("Refnet header KHRQ23M64H", "Nourrice Refnet KHRQ23M64H", "R32", "capacity index 290 to below 640"),
        ("Refnet header KHRQ23M75H", "Nourrice Refnet KHRQ23M75H", "R32", "capacity index 640 or more"),
    ] {
        v.push(m(n, f, C::RefrigerantBranches, K::RefnetHeader).size(700.0, 100.0).daikin(8, &format!("{gas}: up to 8 branches, {limit}; {steps}")));
    }
    // VRV 5 (R32) heat-recovery branch selectors: 4 to 12 ports; header pipes liquid 5/8", HP/LP gas 7/8", suction gas 7/8";
    // branches liquid 3/8" or 1/4", gas 5/8", 1/2" or 3/8"; at most 5 indoor units and index 140 per port (250 with two ports joined).
    let r32 = "Header: Liq 5/8\" · HP/LP 7/8\" · Suc 7/8\". Port: Liq 3/8\" or 1/4\", gas 5/8\", 1/2\" or 3/8\". 5 units / index 140 per port (250 joining 2)";
    for (n, ports) in [("BS4A14AJV1B9", 4), ("BS6A14AJV1B9", 6), ("BS8A14AJV1B9", 8), ("BS10A14AJV1B9", 10), ("BS12A14AJV1B9", 12)] {
        v.push(
            m(n, n, C::BranchBoxes, K::BranchBox)
                .size(300.0 + 100.0 * f64::from(ports), 350.0)
                .daikin(ports, &format!("VRV 5 R32 heat recovery. {r32}")),
        );
    }
    // VRV IV (R410A) heat-recovery branch selectors; the outdoor-side sizes as the data book gives them (two values where
    // the size depends on the capacity).
    let q14 = "Port: Liq 3/8\" or 1/4\", gas 5/8\" or 1/2\"; 5 units / index 140 per port";
    for (n, ports, sizes) in [
        ("BS4Q14AV1B", 4, "Liq 3/8\" · HP/LP 3/4\" or 5/8\" · Suc 7/8\" or 3/4\""),
        ("BS6Q14AV1B", 6, "Liq 1/2\" · HP/LP 3/4\" or 7/8\" · Suc 1 1/8\" or 7/8\""),
        ("BS8Q14AV1B", 8, "Liq 1/2\" or 5/8\" · HP/LP 3/4\", 7/8\" or 1 1/8\" · Suc 1 1/8\""),
        ("BS10Q14AV1B", 10, "Liq 5/8\" · HP/LP 1 1/8\" · Suc 1 1/8\""),
        ("BS12Q14AV1B", 12, "Liq 5/8\" or 3/4\" · HP/LP 1 1/8\" · Suc 1 1/8\" or 1 3/8\""),
        ("BS16Q14AV1B", 16, "Liq 3/4\" · HP/LP 1 1/8\" · Suc 1 3/8\""),
    ] {
        v.push(
            m(n, n, C::BranchBoxes, K::BranchBox)
                .size(300.0 + 80.0 * f64::from(ports), 350.0)
                .daikin(ports, &format!("VRV IV R410A heat recovery. Header: {sizes}. {q14}")),
        );
    }
    for (n, units) in [("BS1Q10A", "6"), ("BS1Q16A", "8"), ("BS1Q25A", "8")] {
        v.push(
            m(n, n, C::BranchBoxes, K::BranchBox)
                .size(400.0, 300.0)
                .daikin(1, &format!("VRV IV R410A heat recovery, single port: liquid 3/8\"; up to {units} indoor units")),
        );
    }
    // Branch providers of the multi systems: 2 or 3 rooms.
    for (n, ports) in [("BPMKS967A2", 2), ("BPMKS967A3", 3)] {
        v.push(
            m(n, n, C::BranchBoxes, K::BranchBox)
                .size(300.0 + 60.0 * f64::from(ports), 300.0)
                .daikin(ports, "Multi branch provider: one outdoor unit to 2 or 3 rooms"),
        );
    }
    v
}
