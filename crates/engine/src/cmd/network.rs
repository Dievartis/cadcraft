//! Pipe networks: PIPENETWORK names the networks (the pipes that are one circuit), gives each a colour and a line type so that they can be
//! told apart on a plan, and changes them; NETWORKLEGEND draws the legend of the networks of the drawing.

use cadcraft_doc::{HvacUnits, NETWORK_LINETYPES, NETWORK_PALETTE, NetworkLegendParams, NetworkSummary};
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

use super::{CommandSpec, bad, str_param};

use crate::{Accept, EngineError, Input, Interactive, Prompt, Result, Session, Step};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("pipenetwork", "Pipe Networks", run_network)
            .menu(&["Draw", "HVAC", "Pipe Networks"])
            .params(
                "{} (name every network that has no name, CHW1, HW1, COND1…, and give each name its colour) → {networks} | {handle | handles (a pipe, a part or a riser), name} (the whole network \
                 of each is given that name) → {changed, name} | {rename: {from, to}} → {changed} | {style: {name, color? (1 to 12: Blue, Orange, Green, Vermillion, Pink, Sky blue, Purple, Brown, Teal, Maroon, Olive, Gold; \
                 a name of these; \"#rrggbb\"; or [r, g, b]), linetype? (CONTINUOUS, NET_DASH, NET_DASHDOT, NET_DOT, NET_LONG, or a line type of the drawing)}} → {style} | {list: true} → {networks, palette, linetypes}. \
                 A network is the pipes of one service whose ends meet, through the parts in the line of a pipe, through the equipment and units they end in, and through the shaft of a riser; a name given to one \
                 spreads to the others, and a pipe drawn on a named network takes its name. Each network has a layer for each layer of its service (M-PIPE-CHWS-CTA1), in its colour and line type. \
                 networks: [{name, services, pipes, length, color, linetype}]",
            ),
        CommandSpec::new("networklegend", "Network Legend", run_legend)
            .menu(&["Draw", "HVAC", "Network Legend"])
            .params("{at ([x, y], the upper left corner), title? (default Networks), labelHeight? (mm)} → {handle}: a row for each network, in its colour and line type; it is redrawn when a network is named, renamed or restyled")
            .interactive(|_| Ok(Box::new(LegendM::default()))),
    ]
}

fn other(m: impl Into<String>) -> EngineError {
    EngineError::Other(m.into())
}

fn doc_err(e: cadcraft_doc::DocError) -> EngineError {
    other(e.to_string())
}

/// What the commands that draw pipes, parts and risers do afterwards: a name that one member of a network has goes to the others. A network
/// that has two names (two named networks that were joined) is told. Nothing is done while nothing is named.
pub(super) fn propagate(s: &mut Session) {
    let warnings = match s.doc_mut().map(|d| d.propagate_pipe_networks()) {
        Ok(Ok((_, w))) => w,
        _ => Vec::new(),
    };
    for w in warnings {
        s.echo(w);
    }
}

fn parse_color(v: &Value) -> Option<[u8; 3]> {
    match v {
        Value::Number(n) => {
            let i = n.as_u64()?;
            (1..=NETWORK_PALETTE.len() as u64).contains(&i).then(|| NETWORK_PALETTE.get(i as usize - 1).map(|p| p.1)).flatten()
        }
        Value::String(t) => {
            let t = t.trim();
            if let Some(hex) = t.strip_prefix('#').or(Some(t)).filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit())) {
                let n = u32::from_str_radix(hex, 16).ok()?;
                return Some([(n >> 16) as u8, (n >> 8) as u8, n as u8]);
            }
            NETWORK_PALETTE.iter().find(|(name, _)| name.eq_ignore_ascii_case(t)).map(|p| p.1)
        }
        Value::Array(a) if a.len() == 3 => {
            let c: Option<Vec<u8>> = a.iter().map(|x| x.as_u64().and_then(|n| u8::try_from(n).ok())).collect();
            c.and_then(|c| <[u8; 3]>::try_from(c).ok())
        }
        _ => None,
    }
}

fn summary_json(n: &NetworkSummary) -> Value {
    let tags: Vec<String> = n
        .services
        .iter()
        .map(|(s, l)| cadcraft_doc::PipeParams { leg: *l, ..cadcraft_doc::PipeParams::new(*s, 50.0, HvacUnits::default()) }.tag().to_string())
        .collect();
    json!({
        "name": n.name, "services": tags, "pipes": n.pipes, "length": n.length_m,
        "color": n.style.as_ref().map(|s| format!("#{:02x}{:02x}{:02x}", s.color[0], s.color[1], s.color[2])),
        "linetype": n.style.as_ref().map(|s| s.linetype.clone()),
    })
}

fn run_network(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "pipenetwork";
    if let Some(r) = p.get("rename").filter(|v| v.is_object()) {
        let (from, to) = (str_param(r, "from"), str_param(r, "to"));
        let (Some(from), Some(to)) = (from, to) else { return Err(bad(CMD, "`rename` needs `from` and `to`")) };
        let changed = s.doc_mut()?.rename_pipe_network(from, to).map_err(doc_err)?;
        return Ok(json!({ "changed": changed }));
    }
    if let Some(st) = p.get("style").filter(|v| v.is_object()) {
        let name = str_param(st, "name").ok_or_else(|| bad(CMD, "`style` needs a `name`"))?;
        let color = st
            .get("color")
            .filter(|v| !v.is_null())
            .map(|v| parse_color(v).ok_or_else(|| bad(CMD, "`color` is a number from 1 to 12, a palette name, \"#rrggbb\" or [r, g, b]")))
            .transpose()?;
        let style = s.doc_mut()?.set_network_style(name, color, str_param(st, "linetype")).map_err(doc_err)?;
        return Ok(
            json!({ "style": { "name": style.name, "color": format!("#{:02x}{:02x}{:02x}", style.color[0], style.color[1], style.color[2]), "linetype": style.linetype } }),
        );
    }
    if p.get("list").and_then(Value::as_bool).unwrap_or(false) {
        let networks: Vec<Value> = s.doc()?.pipe_network_summaries().iter().map(summary_json).collect();
        let palette: Vec<Value> = NETWORK_PALETTE
            .iter()
            .enumerate()
            .map(|(i, (n, c))| json!({ "number": i + 1, "name": n, "color": format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]) }))
            .collect();
        let linetypes: Vec<Value> = NETWORK_LINETYPES.iter().map(|(n, w)| json!({ "name": n, "label": w })).collect();
        return Ok(json!({ "networks": networks, "palette": palette, "linetypes": linetypes }));
    }
    if p.get("handle").is_some() || p.get("handles").is_some() {
        let hs = super::arch::handles_param(CMD, p)?;
        let name = str_param(p, "name").ok_or_else(|| bad(CMD, "`name` is required"))?;
        let changed = s.doc_mut()?.set_pipe_network_name(&hs, name).map_err(doc_err)?;
        return Ok(json!({ "changed": changed, "name": name.trim() }));
    }
    let networks = s.doc_mut()?.name_pipe_networks().map_err(doc_err)?;
    Ok(json!({ "networks": networks.iter().map(summary_json).collect::<Vec<_>>() }))
}

fn legend_at(s: &mut Session, at: Vec2, title: Option<&str>, label: Option<f64>) -> Result<cadcraft_doc::Handle> {
    let mut params = NetworkLegendParams::new(HvacUnits::of(s.doc()?));
    if let Some(t) = title {
        params.title = t.to_string();
    }
    params.label_height_mm = label.unwrap_or(params.label_height_mm);
    params.validate().map_err(doc_err)?;
    let space = s.space();
    let common = s.current_common();
    s.doc_mut()?.add_network_legend(&space, common, &params, at).map_err(doc_err)
}

fn run_legend(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "networklegend";
    let at = super::point_param(p, "at").ok_or_else(|| bad(CMD, "`at` ([x, y]) is required"))?;
    let label = p
        .get("labelHeight")
        .filter(|v| !v.is_null())
        .map(|v| v.as_f64().filter(|x| x.is_finite() && *x > 0.0).ok_or_else(|| bad(CMD, "`labelHeight` must be a number above 0")))
        .transpose()?;
    let h = legend_at(s, at, str_param(p, "title"), label)?;
    Ok(json!({ "handle": h.hex() }))
}

/// NETWORKLEGEND at the command line: a click puts a legend there.
#[derive(Default)]
struct LegendM {
    title: Option<String>,
    ask_title: bool,
}

impl Interactive for LegendM {
    fn name(&self) -> &'static str {
        "NETWORKLEGEND"
    }
    fn prompt(&self, _s: &Session) -> Prompt {
        if self.ask_title {
            return Prompt::new("Enter the title of the legend", Accept::TEXT).default(self.title.clone().unwrap_or_else(|| "Networks".into()));
        }
        Prompt::new("Specify the upper left corner of the legend", Accept::POINT).kw(&["Title"])
    }
    fn input(&mut self, s: &mut Session, i: Input) -> Result<Step> {
        if self.ask_title {
            if let Input::Text(t) | Input::Keyword(t) = &i
                && !t.trim().is_empty()
            {
                self.title = Some(t.trim().to_string());
            }
            self.ask_title = false;
            return Ok(Step::Continue);
        }
        match i {
            Input::Keyword(k) if k == "Title" => {
                self.ask_title = true;
                Ok(Step::Continue)
            }
            Input::Point(at) => {
                legend_at(s, at, self.title.as_deref(), None)?;
                Ok(Step::Done)
            }
            Input::Enter | Input::Cancel => Ok(Step::Cancel),
            _ => Ok(Step::Continue),
        }
    }
}
