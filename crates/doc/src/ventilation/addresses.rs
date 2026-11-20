//! The addresses of the objects that have one: a device on a bus, a thermostat, an indoor unit, an air handling unit, a luminaire or a switch on a KNX or DALI line.
//! The address is written beside the object when the object shows it; the table of addresses lists them all, and two devices on one bus with one address are told.

use cadcraft_geom::Vec2;
use serde::{Deserialize, Serialize};

use super::{ahu_of, control_of, device_of, equipment_of};
use crate::{Handle, Result};

/// One addressed object.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressRow {
    pub handle: String,
    pub tag: String,
    /// What the object is: "Room thermostat", "Cassette", "AHU", "Downlight"…
    pub kind: String,
    pub address: String,
    /// The bus it is on, or the controller it is wired to.
    pub bus: String,
    pub space: String,
    pub shown: bool,
}

impl crate::Drawing {
    /// The objects of model space that have an address, by bus and address.
    pub fn address_rows(&self) -> Vec<AddressRow> {
        let spaces: Vec<(String, Vec<Vec2>)> = self.model.iter().filter_map(|e| self.space_outline(e.handle).map(|(sp, o)| (sp.name, o))).collect();
        let space_of = |at: Vec2| spaces.iter().find(|(_, o)| cadcraft_geom::point_in_polygon(o, at)).map(|(n, _)| n.clone()).unwrap_or_default();
        let mut rows = Vec::new();
        for e in self.model.iter() {
            let crate::EntityKind::Insert(i) = &e.kind else { continue };
            let at = Vec2::new(i.insert.x, i.insert.y);
            let (tag, kind, address, bus, shown) = if let Some(c) = control_of(e) {
                let bus = if c.bus.trim().is_empty() { c.controller.clone() } else { c.bus.clone() };
                (c.name, c.kind.name().to_string(), c.address, bus, c.show_address)
            } else if let Some(q) = equipment_of(e) {
                (q.name, q.kind.name().to_string(), q.address, String::new(), q.show_address)
            } else if let Some(u) = ahu_of(e) {
                (u.name, "AHU".to_string(), u.address, String::new(), u.show_address)
            } else if let Some(d) = device_of(e) {
                (d.name, d.kind.name().to_string(), d.address, String::new(), d.show_address)
            } else {
                continue;
            };
            if address.trim().is_empty() {
                continue;
            }
            rows.push(AddressRow { handle: e.handle.hex(), tag, kind, address: address.trim().to_string(), bus, space: space_of(at), shown });
        }
        rows.sort_by(|a, b| a.bus.to_lowercase().cmp(&b.bus.to_lowercase()).then(a.address.cmp(&b.address)).then(a.tag.cmp(&b.tag)));
        rows
    }

    /// The addresses used twice on one bus (or on one controller): the two tags and the address, for each clash. Only the control devices are compared: the
    /// other objects have no bus of their own to compare on.
    pub fn address_clashes(&self) -> Vec<String> {
        let rows: Vec<AddressRow> = self.address_rows().into_iter().filter(|r| !r.bus.trim().is_empty()).collect();
        let mut out = Vec::new();
        for (i, a) in rows.iter().enumerate() {
            for b in rows.iter().skip(i + 1) {
                if a.bus.eq_ignore_ascii_case(&b.bus) && a.address.eq_ignore_ascii_case(&b.address) {
                    out.push(format!("{} and {} have the address {} on {}", a.tag, b.tag, a.address, a.bus));
                }
            }
        }
        out
    }

    /// Shows or hides the size written beside every equipment, and redraws them. Returns how many changed.
    pub fn set_equipment_size_display(&mut self, show: bool) -> Result<usize> {
        let hs: Vec<Handle> = self.model.iter().map(|e| e.handle).collect();
        let mut n = 0;
        for h in hs {
            let Some(mut q) = self.entity(h).and_then(|e| equipment_of(e)) else { continue };
            if q.show_size != show {
                q.show_size = show;
                self.regenerate_equipment(h, &q)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Shows or hides the address beside every object that can have one, and redraws them. Returns how many objects changed.
    pub fn set_address_display(&mut self, show: bool) -> Result<usize> {
        let hs: Vec<Handle> = self.model.iter().map(|e| e.handle).collect();
        let mut n = 0;
        for h in hs {
            let Some(e) = self.entity(h).cloned() else { continue };
            if let Some(mut c) = control_of(&e)
                && c.show_address != show
            {
                c.show_address = show;
                self.regenerate_control(h, &c)?;
                n += 1;
            } else if let Some(mut q) = equipment_of(&e)
                && q.show_address != show
            {
                q.show_address = show;
                self.regenerate_equipment(h, &q)?;
                n += 1;
            } else if let Some(mut u) = ahu_of(&e)
                && u.show_address != show
            {
                u.show_address = show;
                self.regenerate_ahu(h, &u)?;
                n += 1;
            } else if let Some(mut d) = device_of(&e)
                && d.show_address != show
            {
                d.show_address = show;
                self.regenerate_device(h, &d)?;
                n += 1;
            }
        }
        Ok(n)
    }
}
