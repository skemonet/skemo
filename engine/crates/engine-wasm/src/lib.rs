//! The skemo engine in WebAssembly (F-WASM-1, 2): the same code as the native one, hence the same
//! `layout.json`.
//!
//! ```js
//! import init, { render } from './engine_wasm.js';
//! await init();
//! const r = JSON.parse(render(source, css, JSON.stringify({ theme: 'dark' })));
//! // r.html, r.svg, r.layout, r.diagnostics, r.warnings
//! ```
//!
//! A ready-made element (F-WASM-3, F-ANIM-9): `demo/skemo-view.js` defines
//! `<skemo-view src="diagram.skemo" css="custom.css" step="2" theme="dark">`, with `states()`,
//! `goto(step)` and the `stepchange` event (example: `demo/element.html`).

#![forbid(unsafe_code)]

use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

fn diag_json(src: &str, d: &skemo_lang::Diagnostic) -> Value {
    let (line, col) = skemo_lang::diag::line_col(src, d.span.start);
    json!({
        "severity": if d.is_error() { "error" } else { "warning" },
        "code": d.code,
        "message": d.message,
        "help": d.help,
        "line": line,
        "column": col,
        "start": d.span.start,
        "end": d.span.end,
        "text": skemo_lang::diag::render(src, &skemo_lang::tr!("diagram", "schéma"), d),
    })
}

/// Core of the API, testable natively: returns a JSON object.
pub fn render_json(source: &str, css: &str, options: &str) -> Value {
    let opts: Value = serde_json::from_str(options).unwrap_or(Value::Null);
    let theme = opts
        .get("theme")
        .and_then(Value::as_str)
        .and_then(skemo_lang::style::Theme::parse);
    let mut sheets = Vec::new();
    if !css.trim().is_empty() {
        sheets.push(css.to_string());
    }
    let title = opts
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("skemo")
        .to_string();
    let embed_fonts = opts
        .get("embedFonts")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match skemo_engine::from_source(
        source,
        &skemo_engine::Options {
            css: sheets,
            theme,
            base_theme: None,
            high_contrast: opts
                .get("highContrast")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            transparent: opts
                .get("transparent")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            ..Default::default()
        },
    ) {
        Ok((d, warnings)) => json!({
            "ok": true,
            "html": skemo_engine::html::render(&d, &skemo_engine::html::HtmlOptions { title, embed_fonts }),
            "svg": skemo_engine::svg::render(&d),
            "layout": d.layout_json(),
            "diagnostics": warnings.iter().map(|w| diag_json(source, w)).collect::<Vec<_>>(),
            "warnings": d.warnings,
        }),
        Err(skemo_engine::Error::Source(errs)) => json!({
            "ok": false,
            "diagnostics": errs.iter().map(|e| diag_json(source, e)).collect::<Vec<_>>(),
        }),
        Err(skemo_engine::Error::Internal(m)) => {
            json!({ "ok": false, "error": m, "diagnostics": [] })
        }
    }
}

/// `render(source, css, options)` → JSON string `{ ok, html, svg, layout, diagnostics, warnings }`.
/// Options: `{ "theme": "light|dark|print|auto", "title": "…", "embedFonts": false }`.
#[wasm_bindgen]
pub fn render(source: &str, css: &str, options: &str) -> String {
    render_json(source, css, options).to_string()
}

/// Core of the editing API (F-WASM-4), testable natively.
/// `op`: `{ "op": "move_card", "id": "fw", "x": 120, "y": 40 }`, `release_card`, `set_via`
/// (`points: [[x, y]…]`), `set_side` (`side: "left-top"`), `add_class` / `remove_class`
/// (`target: {"node"|"edge": id}`, `class`), `set_attr` / `remove_attr` (`name`, `value`).
/// Returns `{ ok, source }` (the rest of the file is intact) or `{ ok: false, error }`.
pub fn edit_json(source: &str, op: &str) -> Value {
    use skemo_lang::edit::{self, Target};
    let Ok(o) = serde_json::from_str::<Value>(op) else {
        return json!({ "ok": false, "error": skemo_lang::tr!("unreadable operation (JSON expected)", "opération illisible (JSON attendu)") });
    };
    let s = |k: &str| {
        o.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let f = |k: &str| o.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let target = || match (
        o["target"].get("node").and_then(Value::as_str),
        o["target"].get("edge").and_then(Value::as_str),
    ) {
        (Some(n), _) => Target::Node(n.to_string()),
        (_, Some(e)) => Target::Edge(e.to_string()),
        _ => Target::Diagram,
    };
    let r = match s("op").as_str() {
        "move_card" => edit::move_card(source, &s("id"), f("x"), f("y")),
        "release_card" => edit::release_card(source, &s("id")),
        "set_via" => {
            let pts: Vec<(f64, f64)> = o["points"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?)))
                .collect();
            edit::set_via(source, &s("edge"), &pts)
        }
        "set_side" => edit::set_side(source, &s("edge"), &s("side")),
        "add_class" => edit::add_class(source, &target(), &s("class")),
        "remove_class" => edit::remove_class(source, &target(), &s("class")),
        "set_attr" => edit::set_attr(source, &target(), &s("name"), &s("value")),
        "remove_attr" => edit::remove_attr(source, &target(), &s("name")),
        other => {
            return json!({ "ok": false, "error": skemo_lang::tr!("unknown operation « {other} »", "opération « {other} » inconnue") });
        }
    };
    match r {
        Ok(src) => json!({ "ok": true, "source": src }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

/// `edit(source, op)` → JSON string `{ ok, source }`: surgical editing of the text (F-WASM-4).
/// Chain with `render(source…)` for the new rendering.
#[wasm_bindgen]
pub fn edit(source: &str, op: &str) -> String {
    edit_json(source, op).to_string()
}

/// Editing session (E-ROUTE-3): dragging a card routes its arrows again live.
/// ```js
/// const ed = new Editor(source, css, '{}');
/// const f = JSON.parse(ed.frame());          // framing offset: x_file = x_svg - f.ox
/// ed.drag('fw', 120, 40);                    // file coordinates
/// svgEl.outerHTML = ed.svg();
/// source = ed.commit();                      // writes at="120,40"
/// ```
#[wasm_bindgen]
pub struct Editor {
    session: skemo_engine::session::Session,
}

#[wasm_bindgen]
impl Editor {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, css: &str, options: &str) -> Result<Editor, JsError> {
        let opts: Value = serde_json::from_str(options).unwrap_or(Value::Null);
        let theme = opts
            .get("theme")
            .and_then(Value::as_str)
            .and_then(skemo_lang::style::Theme::parse);
        let css = if css.trim().is_empty() {
            vec![]
        } else {
            vec![css.to_string()]
        };
        let session = skemo_engine::session::Session::new(
            source,
            skemo_engine::Options {
                css,
                theme,
                base_theme: None,
                high_contrast: false,
                transparent: false,
                ..Default::default()
            },
        )
        .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Editor { session })
    }

    /// Moves a card (file coordinates); returns the ids of the arrows routed again (JSON).
    pub fn drag(&mut self, id: &str, x: f64, y: f64) -> Result<String, JsError> {
        let ids = self.session.drag(id, x, y).map_err(|e| JsError::new(&e))?;
        Ok(serde_json::to_string(&ids).unwrap_or_default())
    }

    /// Waypoints of an arrow (`[[x, y], …]` in JSON, file coordinates; `[]` = none); returns the ids of
    /// the arrows routed again (JSON).
    pub fn set_via(&mut self, edge: &str, points: &str) -> Result<String, JsError> {
        let pts: Vec<(f64, f64)> =
            serde_json::from_str(points).map_err(|e| JsError::new(&e.to_string()))?;
        let ids = self
            .session
            .set_via(edge, &pts)
            .map_err(|e| JsError::new(&e))?;
        Ok(serde_json::to_string(&ids).unwrap_or_default())
    }

    /// Sides of an arrow's ends (`"left-top"`, one half may be empty; `""` = auto); returns the ids of
    /// the arrows routed again (JSON).
    pub fn set_side(&mut self, edge: &str, side: &str) -> Result<String, JsError> {
        let ids = self
            .session
            .set_side(edge, side)
            .map_err(|e| JsError::new(&e))?;
        Ok(serde_json::to_string(&ids).unwrap_or_default())
    }

    /// Writes the positions into the source and returns it.
    pub fn commit(&mut self) -> Result<String, JsError> {
        self.session.commit().map_err(|e| JsError::new(&e))
    }

    /// Releases a card (automatic placement); returns the source.
    pub fn release(&mut self, id: &str) -> Result<String, JsError> {
        self.session.release(id).map_err(|e| JsError::new(&e))
    }

    pub fn svg(&self) -> String {
        skemo_engine::svg::render(&self.session.diagram)
    }

    pub fn source(&self) -> String {
        self.session.source.clone()
    }

    /// Cadrage `{ ox, oy, w, h }` : `x_svg` = `x_fichier` + ox.
    pub fn frame(&self) -> String {
        let f = self.session.diagram.frame;
        json!({ "ox": f.ox, "oy": f.oy, "w": f.w, "h": f.h }).to_string()
    }

    /// Boxes of the cards and zones (file coordinates), JSON.
    pub fn boxes(&self) -> String {
        let b: serde_json::Map<String, Value> = self
            .session
            .diagram
            .boxes
            .iter()
            .map(|(k, b)| (k.clone(), json!([b.x, b.y, b.w, b.h])))
            .collect();
        Value::Object(b).to_string()
    }
}

/// Language of the messages (`"en"`, `"fr"`, or a locale name like `"fr_FR.UTF-8"`); English by
/// default. Call it once at start, for instance with `navigator.language`.
#[wasm_bindgen]
pub fn set_language(locale: &str) {
    if let Some(l) = skemo_lang::i18n::Lang::parse(locale) {
        skemo_lang::i18n::set_language(l);
    }
}

/// Version de l'API.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: &str = "card#fw.t0 Pare-feu\n  to(core) transit\ncard#core Switch\n";

    const SRC: &str = "diagram(dir=\"right\")\n  card#a A\n    to(b) lien\n  card#b B\n";

    #[test]
    fn render_gives_every_output_and_honours_options() {
        let r = render_json(
            SRC,
            "",
            r##"{"theme":"dark","title":"T","embedFonts":false}"##,
        );
        assert_eq!(r["ok"], true);
        assert!(r["html"].as_str().unwrap().contains("<title>T</title>"));
        assert!(r["svg"].as_str().unwrap().starts_with("<svg"));
        assert!(r["layout"]["layout"]["nodes"]["a"].is_array());
        assert!(r["html"].as_str().unwrap().contains("theme-dark"));
        let hc = render_json(
            SRC,
            "card { --c: #888 }",
            r#"{"highContrast":true,"transparent":true}"#,
        );
        assert_eq!(hc["ok"], true);
        // unreadable options are ignored
        assert_eq!(render_json(SRC, "", "not json")["ok"], true);
    }

    #[test]
    fn render_reports_source_errors_with_positions() {
        let r = render_json("card#a(\n", "", "{}");
        assert_eq!(r["ok"], false);
        let d = &r["diagnostics"][0];
        assert_eq!(d["severity"], "error");
        assert!(d["line"].as_u64().unwrap() >= 1 && d["text"].as_str().is_some());
        assert!(render(SRC, "", "{}").starts_with('{'));
    }

    fn edit_ok(op: &str) -> String {
        let r = edit_json(SRC, op);
        assert_eq!(r["ok"], true, "{r}");
        r["source"].as_str().unwrap().to_string()
    }

    #[test]
    fn every_edit_operation_is_surgical() {
        assert!(
            edit_ok(r#"{"op":"move_card","id":"a","x":10,"y":20}"#)
                .contains("card#a(at=\"10,20\") A")
        );
        let pinned = edit_json(SRC, r#"{"op":"move_card","id":"a","x":10,"y":20}"#)["source"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            edit_json(&pinned, r#"{"op":"release_card","id":"a"}"#)["source"],
            SRC
        );
        assert!(
            edit_ok(r#"{"op":"set_via","edge":"a->b","points":[[5,6],[7,8]]}"#)
                .contains("via=\"5,6 7,8\"")
        );
        assert!(
            edit_ok(r#"{"op":"set_side","edge":"a->b","side":"left-top"}"#)
                .contains("side=\"left-top\"")
        );
        assert!(
            edit_ok(r#"{"op":"add_class","target":{"node":"a"},"class":"x"}"#)
                .contains("card#a.x A")
        );
        assert_eq!(
            edit_json(
                SRC,
                r#"{"op":"remove_class","target":{"node":"a"},"class":"x"}"#
            )["source"],
            SRC
        );
        assert!(
            edit_ok(r#"{"op":"set_attr","target":{"edge":"a->b"},"name":"label","value":"z"}"#)
                .contains("label=\"z\"")
        );
        assert!(
            edit_ok(r#"{"op":"set_attr","target":{},"name":"dir","value":"down"}"#)
                .contains("dir=\"down\"")
        );
        assert_eq!(
            edit_json(
                SRC,
                r#"{"op":"remove_attr","target":{"node":"a"},"name":"at"}"#
            )["source"],
            SRC
        );
        assert!(edit(SRC, r#"{"op":"release_card","id":"a"}"#).contains("\"ok\":true"));
    }

    #[test]
    fn bad_edits_report_an_error_and_never_panic() {
        for op in [
            "not json",
            r#"{"op":"inconnue"}"#,
            r#"{"op":"move_card","id":"zzz","x":1,"y":2}"#,
            r#"{"op":"add_class","target":{"node":"a"},"class":"bad class"}"#,
            r#"{"op":"set_attr","target":{"node":"a"},"name":"bad name","value":"1"}"#,
        ] {
            let r = edit_json(SRC, op);
            assert_eq!(r["ok"], false, "{op}");
            assert!(r["error"].as_str().is_some_and(|e| !e.is_empty()), "{op}");
        }
        assert_eq!(
            edit_json("card#a(\n", r#"{"op":"release_card","id":"a"}"#)["ok"],
            false
        );
    }

    #[test]
    fn editor_session_drags_and_commits() {
        let mut ed = Editor::new(SRC, "", "{}").ok().unwrap();
        assert!(ed.svg().starts_with("<svg"));
        let moved = ed.drag("b", 400.0, 300.0).ok().unwrap();
        assert!(moved.contains("a->b"), "{moved}");
        let boxes: Value = serde_json::from_str(&ed.boxes()).unwrap();
        assert_eq!(boxes["b"][0], 400.0);
        let frame: Value = serde_json::from_str(&ed.frame()).unwrap();
        assert!(frame["w"].as_f64().unwrap() > 0.0);
        let via = ed.set_via("a->b", "[[200,200]]").ok().unwrap();
        assert!(via.contains("a->b"));
        ed.set_side("a->b", "bottom-").ok().unwrap();
        let src = ed.commit().ok().unwrap();
        assert!(
            src.contains("at=\"400,300\"") && src.contains("via=") && src.contains("side="),
            "{src}"
        );
        assert_eq!(ed.source(), src);
        let freed = ed.release("b").ok().unwrap();
        assert!(!freed.contains("card#b(at="));
        assert!(!version().is_empty());
    }

    #[test]
    fn render_and_edit_legacy() {
        let r = render_json(LEGACY, "", "{}");
        assert_eq!(r["ok"], true);
        assert!(r["svg"].as_str().unwrap().starts_with("<svg"));
        let e = edit_json(LEGACY, r#"{"op":"move_card","id":"fw","x":10.4,"y":20}"#);
        assert_eq!(
            e["source"],
            "card#fw.t0(at=\"10,20\") Pare-feu\n  to(core) transit\ncard#core Switch\n"
        );
        let e = edit_json(
            LEGACY,
            r#"{"op":"set_side","edge":"fw->core","side":"left-top"}"#,
        );
        assert!(
            e["source"]
                .as_str()
                .unwrap()
                .contains("to(core side=\"left-top\")")
        );
        let e = edit_json(
            LEGACY,
            r#"{"op":"add_class","target":{"edge":"fw->core"},"class":"dashed"}"#,
        );
        assert!(
            e["source"]
                .as_str()
                .unwrap()
                .contains("to(core).dashed transit")
        );
        assert_eq!(edit_json(LEGACY, r#"{"op":"nope"}"#)["ok"], false);
        let bad = render_json("card#a\n  to(zz)\n", "", "{}");
        assert_eq!(bad["ok"], false);
        assert_eq!(bad["diagnostics"][0]["code"], "unknown-target");
    }
}
