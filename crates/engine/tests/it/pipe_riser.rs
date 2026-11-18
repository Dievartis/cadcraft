//! PIPERISERADD draws the shaft of a pipe riser with its lines; PIPERISEREDIT changes them.

use cadcraft_doc::{Drawing, Handle, PipeLeg, PipeRiserParams, PipeService, pipe_riser_of};
use cadcraft_engine::Session;
use serde_json::{Value, json};

fn metric() -> Session {
    let mut s = Session::new();
    *s.doc_mut().unwrap() = Drawing::new_metric();
    s
}

fn hd(v: &Value) -> Handle {
    Handle::parse_hex(v.as_str().unwrap()).unwrap()
}

fn riser(s: &Session, h: Handle) -> PipeRiserParams {
    pipe_riser_of(s.doc().unwrap().entity(h).unwrap()).expect("a pipe riser")
}

const SHAFT: [[f64; 2]; 4] = [[0.0, 0.0], [800.0, 0.0], [800.0, 500.0], [0.0, 500.0]];

#[test]
fn piperiseradd_draws_a_shaft_with_its_lines_given_as_tags_or_objects() {
    let mut s = metric();
    let r = s.execute("piperiseradd", &json!({"points": SHAFT, "lines": ["CHW-S", "chw-r", "HW-S", "HW-R", "COND", {"service": "refrigerantTriple", "dn": "3/8", "gas": "7/8", "hpGas": "3/4"}], "height": 3200, "name": "CT7"})).unwrap();
    assert_eq!((r["name"].as_str(), r["lines"].as_array().unwrap().len()), (Some("CT7"), 6));
    let p = riser(&s, hd(&r["handle"]));
    assert_eq!((p.up, p.height_mm), (true, Some(3200.0)));
    let services: Vec<_> = p.lines.iter().map(|l| (l.service, l.leg)).collect();
    assert_eq!(
        &services[..5],
        &[
            (PipeService::ChilledWater, PipeLeg::Supply),
            (PipeService::ChilledWater, PipeLeg::Return),
            (PipeService::HotWater, PipeLeg::Supply),
            (PipeService::HotWater, PipeLeg::Return),
            (PipeService::Condensate, PipeLeg::Supply)
        ]
    );
    // A tag gives the size of its service; a refrigerant link is typed in inches.
    assert_eq!((p.lines[0].dn_mm, p.lines[0].imposed), (50.0, false));
    assert_eq!((p.lines[5].dn_mm, p.lines[5].gas_dn_mm, p.lines[5].gas2_dn_mm), (9.52, 22.2, 19.05));
    // Without lines: chilled water supply and return; the name counts on; the shaft can be a closed polyline.
    let poly = s.execute("pline", &json!({"points": [[2000, 0], [2800, 0], [2800, 500], [2000, 500]], "closed": true})).unwrap();
    let h = hd(poly.get("handle").or_else(|| poly.get("handles").and_then(|a| a.get(0))).unwrap());
    let r2 = s.execute("piperiseradd", &json!({"handle": h.hex()})).unwrap();
    assert_eq!(r2["name"], "CT2");
    assert_eq!(riser(&s, hd(&r2["handle"])).lines.len(), 2);
    for bad in [
        json!({}),
        json!({"points": [[0, 0], [1, 1]]}),
        json!({"points": SHAFT, "lines": ["XYZ"]}),
        json!({"points": SHAFT, "lines": [{"leg": "supply"}]}),
        json!({"points": SHAFT, "lines": [{"service": "hotWater", "dn": "3/8"}]}),
        json!({"points": SHAFT, "height": -4}),
        json!({"points": [[0, 0], [1, 0], [2, 0]]}),
    ] {
        assert!(s.execute("piperiseradd", &bad).is_err(), "{bad}");
    }
}

#[test]
fn piperiseredit_changes_the_riser_and_its_lines() {
    let mut s = metric();
    let r = s.execute("piperiseradd", &json!({"points": SHAFT})).unwrap();
    let h = hd(&r["handle"]);
    s.execute("piperiseredit", &json!({"handle": h.hex(), "name": "CT9", "up": false, "height": 3000, "addLines": ["HW-S", "REF"]})).unwrap();
    let p = riser(&s, h);
    assert_eq!((p.name.as_str(), p.up, p.height_mm, p.lines.len()), ("CT9", false, Some(3000.0), 4));
    // One line: its size typed (kept by sizing), its insulation, what it carries to the other floors.
    s.execute("piperiseredit", &json!({"handle": h.hex(), "line": {"index": 1, "dn": 80, "insulation": 30, "incoming": 150}})).unwrap();
    let l = &riser(&s, h).lines[0];
    assert_eq!((l.dn_mm, l.imposed, l.insulation_mm, l.incoming), (80.0, true, 30.0, Some(150.0)));
    s.execute("piperiseredit", &json!({"handle": h.hex(), "line": {"index": 1, "incoming": null, "imposed": false}})).unwrap();
    assert_eq!((riser(&s, h).lines[0].incoming, riser(&s, h).lines[0].imposed), (None, false));
    // A link in inches.
    s.execute("piperiseredit", &json!({"handle": h.hex(), "line": {"index": 4, "dn": "1/4", "gas": "1/2"}})).unwrap();
    assert_eq!((riser(&s, h).lines[3].dn_mm, riser(&s, h).lines[3].gas_dn_mm), (6.35, 12.7));
    // A line removed; all of them replaced; the height cleared.
    s.execute("piperiseredit", &json!({"handle": h.hex(), "removeLine": 2})).unwrap();
    assert_eq!(riser(&s, h).lines.len(), 3);
    s.execute("piperiseredit", &json!({"handle": h.hex(), "lines": ["CO-S", "CO-R"], "height": null})).unwrap();
    let p = riser(&s, h);
    assert_eq!((p.lines.len(), p.height_mm, p.lines[0].service), (2, None, PipeService::ChangeOver));
    for bad in [
        json!({"handle": h.hex(), "removeLine": 9}),
        json!({"handle": h.hex(), "removeLine": 0}),
        json!({"handle": h.hex(), "line": {"index": 7, "dn": 50}}),
        json!({"handle": h.hex(), "line": {"dn": 50}}),
        json!({"handle": h.hex(), "line": {"index": 1, "dn": -2}}),
        json!({"handle": h.hex(), "lines": ["nope"]}),
        json!({"handle": "0"}),
    ] {
        assert!(s.execute("piperiseredit", &bad).is_err(), "{bad}");
    }
}

#[test]
fn piperiseradd_at_the_command_line_takes_the_options_then_the_points() {
    let mut s = metric();
    for t in ["piperiseradd", "n", "COL A", "l", "CHW-S, CHW-R, COND", "h", "3100", "0,0", "800,0", "800,500", "0,500", ""] {
        s.cmdline(t).unwrap();
    }
    assert!(s.running.is_none());
    let p = s.doc().unwrap().model.iter().filter_map(|e| pipe_riser_of(e)).next().expect("a riser");
    assert_eq!((p.name.as_str(), p.lines.len(), p.height_mm), ("COL A", 3, Some(3100.0)));
    // Up toggled.
    for t in ["piperiseradd", "u", "0,2000", "800,2000", "800,2500", ""] {
        s.cmdline(t).unwrap();
    }
    let all: Vec<_> = s.doc().unwrap().model.iter().filter_map(|e| pipe_riser_of(e)).collect();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|r| !r.up));
}

#[test]
fn pipesize_reads_a_network_through_a_riser_and_sizes_its_lines() {
    let mut s = metric();
    let r = s
        .execute("piperiseradd", &json!({"points": [[0, -200], [600, -200], [600, 200], [0, 200]], "lines": ["CHW-S", "CHW-R"], "height": 3200}))
        .unwrap();
    let h = hd(&r["handle"]);
    s.execute("piperiseredit", &json!({"handle": h.hex(), "line": {"index": 1, "incoming": 40}})).unwrap();
    s.execute("pipeadd", &json!({"points": [[300, 0], [5000, 0]], "service": "chilledWater"})).unwrap();
    s.execute("pipeadd", &json!({"points": [[5000, 0], [5000, 3000]], "service": "chilledWater", "load": 25})).unwrap();
    s.execute("pipeadd", &json!({"points": [[5000, 0], [5000, -3000]], "service": "chilledWater", "load": 25})).unwrap();
    let sized = s.execute("pipesize", &json!({})).unwrap();
    assert_eq!(sized["networks"].as_array().unwrap().len(), 1);
    assert_eq!(sized["networks"][0]["totalLoad"], 50.0);
    let line = &sized["risers"][0]["lines"][0];
    assert_eq!((line["tag"].as_str(), line["load"].as_f64()), (Some("CHW-S"), Some(90.0)));
    assert!(line["dn"].as_f64().unwrap() >= 65.0 && line["flowM3h"].as_f64().is_some());
    assert!(sized["risers"][0]["lines"][1]["load"].is_null(), "the return carries nothing here");
    // The line is what the riser now says.
    assert_eq!(riser(&s, h).lines[0].load, Some(90.0));
}

#[test]
fn each_line_of_a_pipe_riser_can_be_given_its_own_direction_and_a_drain_always_goes_down() {
    let mut s = metric();
    let r = s
        .execute(
            "piperiseradd",
            &json!({"points": [[0, 0], [600, 0], [600, 400], [0, 400]], "name": "CT1", "lines": [
                {"service": "chilledWater", "leg": "supply", "direction": "down"},
                "CHW-R",
                "COND",
            ]}),
        )
        .unwrap();
    let h = hd(&r["handle"]);
    let goes = |s: &Session| riser(s, h).lines.iter().map(|l| l.goes_up).collect::<Vec<_>>();
    assert_eq!(goes(&s), [Some(false), None, None]);
    s.execute("piperiseredit", &json!({"handle": r["handle"], "line": {"index": 2, "direction": "up"}})).unwrap();
    s.execute("piperiseredit", &json!({"handle": r["handle"], "line": {"index": 1, "direction": null}})).unwrap();
    assert_eq!(goes(&s), [None, Some(true), None]);
    // A drain can't be made to go up, nor a direction be anything else.
    assert!(s.execute("piperiseredit", &json!({"handle": r["handle"], "line": {"index": 3, "direction": "up"}})).is_err());
    assert!(s.execute("piperiseredit", &json!({"handle": r["handle"], "line": {"index": 1, "direction": "sideways"}})).is_err());
    assert!(
        s.execute("piperiseradd", &json!({"points": [[0, 0], [600, 0], [600, 400]], "lines": [{"service": "condensate", "direction": "up"}]}))
            .is_err()
    );
    s.execute("piperiseredit", &json!({"handle": r["handle"], "line": {"index": 3, "direction": "down"}})).unwrap();
}
