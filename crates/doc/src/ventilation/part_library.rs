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
    pub height_mm: f64,
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
            height_mm: kind.default_height_mm(),
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

    fn height(mut self, height_mm: f64) -> PartModel {
        self.height_mm = height_mm;
        self
    }

    /// A refrigerant part: the number of outlets and the connections (no maker is named: the sizes follow the makers' public
    /// installation manuals, and the part is a generic one).
    fn branch(mut self, outlets: u32, connections: &str) -> PartModel {
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
            height_mm: Some(self.height_mm),
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
            PartCategory::RefrigerantBranches => "PF",
            PartCategory::BranchBoxes => "BD",
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
        m("Buffer tank 300 l", "Ballon tampon 300 l", C::VesselsAndTanks, K::Tank).volume(300.0).size(600.0, 600.0).height(1300.0),
        m("Buffer tank 500 l", "Ballon tampon 500 l", C::VesselsAndTanks, K::Tank).volume(500.0).size(750.0, 750.0).height(1600.0),
        m("Buffer tank 1000 l", "Ballon tampon 1000 l", C::VesselsAndTanks, K::Tank).volume(1000.0).size(950.0, 950.0).height(1900.0),
        m("Decoupling bottle", "Bouteille de découplage", C::VesselsAndTanks, K::DecouplingBottle).dn(80.0),
        m("Domestic hot water cylinder 300 l", "Ballon d'eau chaude sanitaire 300 l", C::VesselsAndTanks, K::Tank)
            .volume(300.0)
            .size(650.0, 650.0)
            .height(1300.0),
        m("Water softener", "Adoucisseur", C::VesselsAndTanks, K::Tank).size(400.0, 400.0).height(1000.0),
        m("Glycol filling unit", "Station de remplissage glycol", C::VesselsAndTanks, K::Tank).size(500.0, 500.0),
        m("Pressurisation unit", "Groupe de maintien de pression", C::VesselsAndTanks, K::ExpansionVessel).size(500.0, 500.0),
        // Heat sources.
        m("Wall-hung gas boiler", "Chaudière gaz murale", C::HeatSources, K::Boiler).power(24.0).size(450.0, 350.0).height(750.0),
        m("Floor-standing condensing boiler", "Chaudière à condensation au sol", C::HeatSources, K::Boiler)
            .power(100.0)
            .size(900.0, 600.0)
            .height(1300.0),
        m("Electric boiler", "Chaudière électrique", C::HeatSources, K::Boiler).power(18.0).size(450.0, 350.0).height(750.0),
        m("Plate heat exchanger", "Échangeur à plaques", C::HeatSources, K::HeatExchanger).power(100.0),
        m("Wood pellet boiler", "Chaudière à granulés", C::HeatSources, K::Boiler).power(30.0).size(900.0, 700.0).height(1400.0),
        m("Dry cooler", "Aéroréfrigérant", C::HeatSources, K::HeatExchanger).power(150.0).size(2000.0, 1000.0).height(1100.0),
    ];
    v.extend(refrigerant_models());
    v
}

/// The refrigerant branches (Y joints and headers) and the branch boxes of a variable-refrigerant-flow system. No maker's name
/// or reference is written: the plan says what the part is. The sizes and the number of outlets follow the makers' public
/// installation manuals.
fn refrigerant_models() -> Vec<PartModel> {
    use PartCategory as C;
    use PartKind as K;
    let m = PartModel::new;
    let steps = "sockets stepped: cut to the pipe size";
    let mut v = Vec::new();
    // Y joints: the model follows the capacity index of the units downstream.
    for (name, name_fr, limit) in [
        ("Refrigerant Y joint, index below 200", "Piquage frigorifique Y, indice inférieur à 200", "capacity index below 200"),
        ("Refrigerant Y joint, index 200 to 290", "Piquage frigorifique Y, indice 200 à 290", "capacity index 200 to below 290"),
        ("Refrigerant Y joint, index 290 to 640", "Piquage frigorifique Y, indice 290 à 640", "capacity index 290 to below 640"),
        ("Refrigerant Y joint, index 640 or more", "Piquage frigorifique Y, indice 640 et plus", "capacity index 640 or more"),
    ] {
        v.push(m(name, name_fr, C::RefrigerantBranches, K::RefnetJoint).branch(2, &format!("{limit}; {steps}")));
    }
    // Headers (clarinettes, nourrices): up to 8 outlets, the model following the capacity index below.
    for (name, name_fr, limit) in [
        ("Refrigerant header, index below 290", "Nourrice frigorifique (clarinette), indice inférieur à 290", "capacity index below 290"),
        ("Refrigerant header, index 290 to 640", "Nourrice frigorifique (clarinette), indice 290 à 640", "capacity index 290 to below 640"),
        (
            "Refrigerant header, index 640 or more",
            "Nourrice frigorifique (clarinette), indice 640 et plus",
            "capacity index 640 or more (needed when the pipe above is 1 3/8\" or larger)",
        ),
    ] {
        v.push(m(name, name_fr, C::RefrigerantBranches, K::RefnetHeader).size(700.0, 100.0).branch(8, &format!("up to 8 outlets; {limit}; {steps}")));
    }
    // Heat-recovery branch boxes for R32: 4 to 12 outlets; header pipes liquid 5/8", HP/LP gas 7/8", suction gas 7/8"; outlets
    // liquid 3/8" or 1/4", gas 5/8", 1/2" or 3/8"; at most 5 indoor units and index 140 per outlet (250 with two outlets joined).
    let r32 = "Header: liquid 5/8\" · HP/LP gas 7/8\" · suction gas 7/8\". Outlet: liquid 3/8\" or 1/4\", gas 5/8\", 1/2\" or 3/8\". 5 units / index 140 per outlet (250 joining 2)";
    for (n, n_fr, ports) in [
        ("Branch box, 4 outlets (R32 heat recovery)", "Boîte de détente 4 sorties (R32, récupération d'énergie)", 4),
        ("Branch box, 6 outlets (R32 heat recovery)", "Boîte de détente 6 sorties (R32, récupération d'énergie)", 6),
        ("Branch box, 8 outlets (R32 heat recovery)", "Boîte de détente 8 sorties (R32, récupération d'énergie)", 8),
        ("Branch box, 10 outlets (R32 heat recovery)", "Boîte de détente 10 sorties (R32, récupération d'énergie)", 10),
        ("Branch box, 12 outlets (R32 heat recovery)", "Boîte de détente 12 sorties (R32, récupération d'énergie)", 12),
    ] {
        let len = K::BranchBox.length_for_outlets_mm(ports).unwrap_or(700.0);
        v.push(m(n, n_fr, C::BranchBoxes, K::BranchBox).size(len, 350.0).height(300.0).branch(ports, r32));
    }
    // The same for R410A; the header sizes as the data book gives them (two values where the size depends on the capacity).
    let q14 = "Outlet: liquid 3/8\" or 1/4\", gas 5/8\" or 1/2\"; 5 units / index 140 per outlet";
    for (n, n_fr, ports, sizes) in [
        (
            "Branch box, 4 outlets (R410A heat recovery)",
            "Boîte de détente 4 sorties (R410A, récupération d'énergie)",
            4,
            "liquid 3/8\" · HP/LP gas 3/4\" or 5/8\" · suction gas 7/8\" or 3/4\"",
        ),
        (
            "Branch box, 6 outlets (R410A heat recovery)",
            "Boîte de détente 6 sorties (R410A, récupération d'énergie)",
            6,
            "liquid 1/2\" · HP/LP gas 3/4\" or 7/8\" · suction gas 1 1/8\" or 7/8\"",
        ),
        (
            "Branch box, 8 outlets (R410A heat recovery)",
            "Boîte de détente 8 sorties (R410A, récupération d'énergie)",
            8,
            "liquid 1/2\" or 5/8\" · HP/LP gas 3/4\", 7/8\" or 1 1/8\" · suction gas 1 1/8\"",
        ),
        (
            "Branch box, 10 outlets (R410A heat recovery)",
            "Boîte de détente 10 sorties (R410A, récupération d'énergie)",
            10,
            "liquid 5/8\" · HP/LP gas 1 1/8\" · suction gas 1 1/8\"",
        ),
        (
            "Branch box, 12 outlets (R410A heat recovery)",
            "Boîte de détente 12 sorties (R410A, récupération d'énergie)",
            12,
            "liquid 5/8\" or 3/4\" · HP/LP gas 1 1/8\" · suction gas 1 1/8\" or 1 3/8\"",
        ),
        (
            "Branch box, 16 outlets (R410A heat recovery)",
            "Boîte de détente 16 sorties (R410A, récupération d'énergie)",
            16,
            "liquid 3/4\" · HP/LP gas 1 1/8\" · suction gas 1 3/8\"",
        ),
    ] {
        let len = K::BranchBox.length_for_outlets_mm(ports).unwrap_or(700.0);
        v.push(m(n, n_fr, C::BranchBoxes, K::BranchBox).size(len, 350.0).height(300.0).branch(ports, &format!("Header: {sizes}. {q14}")));
    }
    for (n, n_fr, units) in [
        (
            "Branch box, 1 outlet (R410A heat recovery, up to 6 units)",
            "Boîte de détente 1 sortie (R410A, récupération d'énergie, 6 unités au plus)",
            "6",
        ),
        (
            "Branch box, 1 outlet (R410A heat recovery, up to 8 units)",
            "Boîte de détente 1 sortie (R410A, récupération d'énergie, 8 unités au plus)",
            "8",
        ),
    ] {
        v.push(
            m(n, n_fr, C::BranchBoxes, K::BranchBox)
                .size(400.0, 300.0)
                .height(250.0)
                .branch(1, &format!("Single outlet: liquid 3/8\"; up to {units} indoor units")),
        );
    }
    // Connection boxes of the multi systems: 2 or 3 rooms.
    for (n, n_fr, ports) in [
        ("Connection box, 2 outlets (multi)", "Boîte de raccordement 2 sorties (multi-split)", 2),
        ("Connection box, 3 outlets (multi)", "Boîte de raccordement 3 sorties (multi-split)", 3),
    ] {
        let len = K::BranchBox.length_for_outlets_mm(ports).unwrap_or(400.0);
        v.push(m(n, n_fr, C::BranchBoxes, K::BranchBox).size(len, 300.0).height(250.0).branch(ports, "One outdoor unit to 2 or 3 rooms"));
    }
    v
}
