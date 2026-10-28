//! Minimal WebAssembly example: `route(JSON scene)` → JSON routes.
//!
//! Input: the format of the test sets (`fixtures/*.json`): `params`, `obstacles`
//! (`{id, rect: [x, y, w, h], kind: "shape" | "region"}`) and `connectors`
//! (`{id, from, to, checkpoints?, passThrough?}`, an end being `{shape, side?}` or `{point: [x, y]}`).
//! Output: `{ routes: { id: [[x, y], …] }, diagnostics: […], ms }`.

#![forbid(unsafe_code)]

use serde_json::{Value, json};
use skemo_router::{Connector, End, Obstacle, Params, Rect, Side, route as route_scene};
use wasm_bindgen::prelude::*;

fn side(s: &str) -> Option<Side> {
    match s {
        "top" => Some(Side::Top),
        "bottom" => Some(Side::Bottom),
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        _ => None,
    }
}

fn end(v: &Value) -> End {
    if let Some(p) = v["point"].as_array() {
        let f = |i: usize| p.get(i).and_then(Value::as_f64).unwrap_or(0.0);
        return End::Point { x: f(0), y: f(1) };
    }
    End::Shape {
        id: v["shape"].as_str().unwrap_or_default().to_string(),
        side: v["side"].as_str().and_then(side),
    }
}

/// Reads a scene in the test sets' format.
fn scene(j: &Value) -> (Vec<Obstacle>, Vec<Connector>, Params) {
    let mut params = Params::default();
    let p = &j["params"];
    let get = |k: &str, d: f64| p[k].as_f64().unwrap_or(d);
    params.shape_buffer = get("shapeBuffer", params.shape_buffer);
    params.ideal_nudging = get("idealNudging", params.ideal_nudging);
    params.segment_penalty = get("segmentPenalty", params.segment_penalty);
    params.crossing_penalty = get("crossingPenalty", params.crossing_penalty);
    let empty = Vec::new();
    let obstacles = j["obstacles"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|o| {
            let r = |i: usize| o["rect"][i].as_f64().unwrap_or(0.0);
            let rect = Rect::new(r(0), r(1), r(2), r(3));
            let id = o["id"].as_str().unwrap_or_default();
            if o["kind"] == "region" {
                Obstacle::region(id, rect)
            } else {
                Obstacle::shape(id, rect)
            }
        })
        .collect();
    let connectors = j["connectors"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|c| {
            let mut k = Connector::new(
                c["id"].as_str().unwrap_or_default(),
                end(&c["from"]),
                end(&c["to"]),
            );
            k.checkpoints = c["checkpoints"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .map(|v| (v[0].as_f64().unwrap_or(0.0), v[1].as_f64().unwrap_or(0.0)))
                .collect();
            k.pass_through = c["passThrough"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            k
        })
        .collect();
    (obstacles, connectors, params)
}

/// Routes a scene (JSON); returns the routes and the diagnostics (JSON).
#[wasm_bindgen]
pub fn route(input: &str) -> String {
    let j: Value = match serde_json::from_str(input) {
        Ok(j) => j,
        Err(e) => return json!({ "error": e.to_string() }).to_string(),
    };
    let (obstacles, connectors, params) = scene(&j);
    let (routes, diags) = route_scene(&obstacles, &connectors, &params);
    let routes: serde_json::Map<String, Value> = routes
        .into_iter()
        .map(|r| {
            let pts = r.points.iter().map(|p| json!([p.0, p.1])).collect();
            (r.id, Value::Array(pts))
        })
        .collect();
    let diags: Vec<String> = diags.iter().map(|d| format!("{d:?}")).collect();
    json!({ "routes": routes, "diagnostics": diags }).to_string()
}
