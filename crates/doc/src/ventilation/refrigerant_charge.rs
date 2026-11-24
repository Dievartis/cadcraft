//! The charge of a refrigerant system against the rooms it serves (NF EN 378-1, annex C: the charge of a system whose indoor units are in an occupied room
//! may not exceed the practical limit of the refrigerant, kg of fluid per m³ of the smallest room, so that a leak cannot reach a concentration that harms
//! the people in it).
//!
//! Only what has been checked against a published source is here: the practical limit of R410A (an A1 fluid), 0.44 kg/m³, which the manufacturers' guides
//! give as the toxicity limit of EN 378. Another refrigerant (the mildly flammable R32 has other rules, with the height of the unit and the floor area) is
//! reported as not checked, to be taken from the manufacturer's manual and from the standard. The charge itself comes from the manufacturer's data, typed on
//! the outdoor unit, and the room's height is the one asked for (a space has no height of its own): the check helps a design, the standard and the
//! manufacturer decide.

use cadcraft_geom::Vec2;

use super::{EquipmentKind, equipment_of};
use crate::{EntityKind, Handle, Result};

/// The practical limit of a refrigerant, kg per m³ of room, when it is known here.
pub fn practical_limit_kg_m3(refrigerant: &str) -> Option<f64> {
    let t: String = refrigerant.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
    match t.as_str() {
        "R410A" => Some(0.44),
        _ => None,
    }
}

/// One room a system serves.
#[derive(Clone, Debug, PartialEq)]
pub struct ChargeRoom {
    pub space: String,
    pub volume_m3: f64,
    /// The most the system may hold for this room, kg.
    pub limit_kg: f64,
    pub ok: bool,
}

/// One refrigerant system (an outdoor unit with a charge typed) against its rooms.
#[derive(Clone, Debug, PartialEq)]
pub struct ChargeReport {
    pub outdoor: Handle,
    pub name: String,
    pub refrigerant: String,
    pub charge_kg: f64,
    pub limit_kg_m3: Option<f64>,
    pub rooms: Vec<ChargeRoom>,
    pub warnings: Vec<String>,
}

impl crate::Drawing {
    /// The charge of each refrigerant system of model space whose outdoor unit has one typed, against the rooms of its indoor units, which are `height_m`
    /// high. A system with no charge typed is not looked at.
    pub fn check_refrigerant_charge(&self, height_m: f64) -> Result<Vec<ChargeReport>> {
        if !(height_m.is_finite() && height_m > 0.0 && height_m <= 100.0) {
            return Err(super::invalid("The height of the rooms is above 0 and at most 100 m."));
        }
        let network = self.check_refrigerant()?;
        let spaces: Vec<(super::SpaceParams, Vec<Vec2>)> = self.model.iter().filter_map(|e| self.space_outline(e.handle)).collect();
        let mut out = Vec::new();
        for n in &network.networks {
            let Some(q) = self.entity(n.outdoor).and_then(|e| equipment_of(e)) else { continue };
            let Some(charge) = q.charge_kg else { continue };
            let mut r = ChargeReport {
                outdoor: n.outdoor,
                name: q.name.clone(),
                refrigerant: q.refrigerant.clone(),
                charge_kg: charge,
                limit_kg_m3: practical_limit_kg_m3(&q.refrigerant),
                rooms: Vec::new(),
                warnings: Vec::new(),
            };
            if !matches!(q.kind, EquipmentKind::VrfOutdoor | EquipmentKind::SplitOutdoor) {
                continue;
            }
            let Some(limit) = r.limit_kg_m3 else {
                r.warnings.push(if q.refrigerant.trim().is_empty() {
                    "the refrigerant is not typed on the outdoor unit".to_string()
                } else {
                    format!(
                        "{}: no practical limit is known here (only R410A is checked): take it from the manufacturer's manual and from NF EN 378-1",
                        q.refrigerant
                    )
                });
                out.push(r);
                continue;
            };
            for h in &n.unit_handles {
                let Some(EntityKind::Insert(i)) = self.entity(*h).map(|e| &e.kind) else { continue };
                let at = Vec2::new(i.insert.x, i.insert.y);
                match spaces.iter().find(|(_, outline)| cadcraft_geom::point_in_polygon(outline, at)) {
                    Some((sp, _)) => {
                        if !r.rooms.iter().any(|x| x.space == sp.name) {
                            let volume = sp.area_m2() * height_m;
                            let max = limit * volume;
                            r.rooms.push(ChargeRoom {
                                space: sp.name.clone(),
                                volume_m3: (volume * 100.0).round() / 100.0,
                                limit_kg: (max * 100.0).round() / 100.0,
                                ok: charge <= max,
                            });
                        }
                    }
                    None => r.warnings.push(format!("an indoor unit ({}) is in no space: its room cannot be checked", h.hex())),
                }
            }
            if r.rooms.is_empty() && r.warnings.is_empty() {
                r.warnings.push("no space is drawn around the indoor units of this system".into());
            }
            for room in r.rooms.iter().filter(|x| !x.ok) {
                r.warnings.push(format!(
                    "the charge ({charge} kg of {}) is above what {} may hold ({} kg for {} m³, {limit} kg/m³): the room should be at least {:.1} m³",
                    q.refrigerant,
                    room.space,
                    room.limit_kg,
                    room.volume_m3,
                    charge / limit
                ));
            }
            out.push(r);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_limits_that_were_checked_are_known() {
        assert_eq!(practical_limit_kg_m3("R410A"), Some(0.44));
        assert_eq!(practical_limit_kg_m3(" r-410a "), Some(0.44));
        assert_eq!(practical_limit_kg_m3("R32"), None);
        assert_eq!(practical_limit_kg_m3(""), None);
    }
}
