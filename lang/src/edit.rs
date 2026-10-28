//! Surgical editing (F-PARSE-7, F-PARSE-10): each operation replaces only the bytes concerned;
//! comments, the order of the lines and the indentation stay intact.
//!
//! ```
//! let src = "card#fw.t0 Firewall\n  to(core) transit\ncard#core Switch\n";
//! let out = skemo_lang::edit::move_card(src, "fw", 120.4, 40.0).unwrap();
//! assert_eq!(out, "card#fw.t0(at=\"120,40\") Firewall\n  to(core) transit\ncard#core Switch\n");
//! ```

use crate::parse::{Located, parse};
use crate::tr;

/// Element an edit is aimed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `diagram(...)`.
    Diagram,
    /// Card or zone, by id.
    Node(String),
    /// Arrow, by id (`from->to`). A tag `to(a b)` carries two arrows: the edit touches the tag, hence
    /// both.
    Edge(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The source holds errors: an invalid file is not edited.
    Invalid,
    /// Element not found.
    NotFound(String),
    /// Invalid value for this attribute.
    BadValue(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::Invalid => write!(
                f,
                "{}",
                tr!(
                    "the source contains errors",
                    "la source contient des erreurs"
                )
            ),
            EditError::NotFound(t) => write!(
                f,
                "{}",
                tr!("element « {t} » not found", "élément « {t} » introuvable")
            ),
            EditError::BadValue(m) => write!(f, "{m}"),
        }
    }
}

pub(crate) fn locate(src: &str, target: &Target) -> Result<Located, EditError> {
    let doc = parse(src);
    if doc.has_errors() {
        return Err(EditError::Invalid);
    }
    let found = match target {
        Target::Diagram => doc.map.diagram.clone(),
        Target::Node(id) => doc.map.nodes.get(id).cloned(),
        Target::Edge(id) => doc.map.edges.get(id).cloned(),
    };
    found.ok_or_else(|| {
        EditError::NotFound(match target {
            Target::Diagram => "diagram".into(),
            Target::Node(i) | Target::Edge(i) => i.clone(),
        })
    })
}

pub(crate) fn splice(src: &str, start: usize, end: usize, with: &str) -> String {
    let mut out = String::with_capacity(src.len() + with.len());
    out.push_str(&src[..start]);
    out.push_str(with);
    out.push_str(&src[end..]);
    out
}

/// Valeur d'attribut entre guillemets (doubles, sauf si elle en contient).
pub(crate) fn quote(v: &str) -> String {
    if v.contains('"') && !v.contains('\'') {
        format!("'{v}'")
    } else {
        format!("\"{}\"", v.replace('"', "\\\""))
    }
}

/// End of the tag's "head" (name, `#id`, classes written before the attribute group).
fn head_end(l: &Located) -> usize {
    let limit = l.attr_group.map_or(usize::MAX, |g| g.start);
    let mut e = l.name.end;
    if let Some(i) = l.id
        && i.end <= limit
    {
        e = e.max(i.end);
    }
    for (_, s) in &l.classes {
        if s.end <= limit {
            e = e.max(s.end);
        }
    }
    e
}

/// Fixe (ou ajoute) l'attribut `name="value"`.
pub fn set_attr(src: &str, target: &Target, name: &str, value: &str) -> Result<String, EditError> {
    if name.is_empty() || name.contains([' ', '(', ')', '=', '"', '\'']) {
        return Err(EditError::BadValue(tr!(
            "invalid attribute name: « {name} »",
            "nom d'attribut invalide : « {name} »"
        )));
    }
    let l = locate(src, target)?;
    let q = quote(value);
    if let Some(a) = l.attr(name) {
        return Ok(match a.value_span {
            Some(v) => splice(src, v.start, v.end, &q),
            None => splice(src, a.span.end, a.span.end, &format!("={q}")),
        });
    }
    Ok(if let Some(g) = l.attr_group {
        let inner = &src[g.start + 1..g.end - 1];
        let sep = if inner.trim().is_empty() { "" } else { " " };
        splice(src, g.end - 1, g.end - 1, &format!("{sep}{name}={q}"))
    } else {
        let at = head_end(&l);
        splice(src, at, at, &format!("({name}={q})"))
    })
}

/// Removes attribute `name` (and the group if it becomes empty).
pub fn remove_attr(src: &str, target: &Target, name: &str) -> Result<String, EditError> {
    let l = locate(src, target)?;
    let Some(a) = l.attr(name) else {
        return Ok(src.to_string());
    };
    let g = l
        .attr_group
        .ok_or_else(|| EditError::NotFound(name.into()))?;
    if l.attrs.len() == 1 {
        return Ok(splice(src, g.start, g.end, ""));
    }
    // remove the separators before it too (or after it, for the first attribute)
    let mut s = a.span.start;
    while s > g.start + 1 && matches!(src.as_bytes()[s - 1], b' ' | b',' | b'\t') {
        s -= 1;
    }
    let mut e = a.span.end;
    if s == g.start + 1 {
        while e < g.end - 1 && matches!(src.as_bytes()[e], b' ' | b',' | b'\t') {
            e += 1;
        }
    }
    Ok(splice(src, s, e, ""))
}

fn valid_class(c: &str) -> bool {
    !c.is_empty()
        && c.chars()
            .all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_')
}

/// Adds class `class` (no effect if it is already there).
pub fn add_class(src: &str, target: &Target, class: &str) -> Result<String, EditError> {
    if !valid_class(class) {
        return Err(EditError::BadValue(tr!(
            "invalid class: « {class} »",
            "classe invalide : « {class} »"
        )));
    }
    let l = locate(src, target)?;
    if l.classes.iter().any(|(c, _)| c == class) {
        return Ok(src.to_string());
    }
    // for `to`, canonical form `to(target).class`: after the attribute group
    let is_to = &src[l.name.start..l.name.end] == "to";
    let at = match (l.classes.last(), l.attr_group) {
        (Some((_, sp)), _) => sp.end,
        (None, Some(g)) if is_to => g.end,
        _ => head_end(&l),
    };
    Ok(splice(src, at, at, &format!(".{class}")))
}

/// Removes class `class`.
pub fn remove_class(src: &str, target: &Target, class: &str) -> Result<String, EditError> {
    let l = locate(src, target)?;
    match l.classes.iter().find(|(c, _)| c == class) {
        Some((_, s)) => Ok(splice(src, s.start - 1, s.end, "")),
        None => Ok(src.to_string()),
    }
}

/// Rounding of a coordinate written by an edit (F-PARSE-10): an integer, or a multiple of `grid`.
pub fn snap(v: f64, grid: Option<f64>) -> i64 {
    match grid.filter(|g| *g > 0.0) {
        Some(g) => ((v / g).round() * g).round() as i64,
        None => v.round() as i64,
    }
}

/// Pins card `id` at `(x, y)` (= `set_attr(at)`, integer values).
pub fn move_card(src: &str, id: &str, x: f64, y: f64) -> Result<String, EditError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(EditError::BadValue(tr!(
            "coordinates are not finite",
            "coordonnées non finies"
        )));
    }
    set_attr(
        src,
        &Target::Node(id.to_string()),
        "at",
        &format!("{},{}", snap(x, None), snap(y, None)),
    )
}

/// Releases card `id` (back to automatic placement).
pub fn release_card(src: &str, id: &str) -> Result<String, EditError> {
    remove_attr(src, &Target::Node(id.to_string()), "at")
}

/// Waypoints of an arrow (`via`), empty = removed.
pub fn set_via(src: &str, edge: &str, points: &[(f64, f64)]) -> Result<String, EditError> {
    let t = Target::Edge(edge.to_string());
    if points.is_empty() {
        return remove_attr(src, &t, "via");
    }
    let v: Vec<String> = points
        .iter()
        .map(|(x, y)| format!("{},{}", snap(*x, None), snap(*y, None)))
        .collect();
    set_attr(src, &t, "via", &v.join(" "))
}

/// Appearance marker of an element (`@2`, `@2-4:up`…): `marker` is written without the `@`
/// (`Some("2")`); `None` removes the appearance. `:focus` markers are kept. A tag `to(a b)` carries
/// two arrows: the edit touches the tag, hence both.
pub fn set_appear(src: &str, target: &Target, marker: Option<&str>) -> Result<String, EditError> {
    let l = locate(src, target)?;
    let appear: Vec<_> = l
        .markers
        .iter()
        .filter(|m| !src[m.start..m.end].ends_with(":focus"))
        .collect();
    let mut out = src.to_string();
    // from the end to the start: the positions stay valid
    let mut spans: Vec<(usize, usize)> = appear.iter().map(|m| (m.start, m.end)).collect();
    spans.sort_by(|a, b| b.cmp(a));
    for (start, end) in spans {
        let from = out[..start].trim_end_matches([' ', '\t']).len();
        out = splice(&out, from, end, "");
    }
    let Some(m) = marker else { return Ok(out) };
    if m.is_empty() || m.contains(char::is_whitespace) {
        return Err(EditError::BadValue(tr!(
            "invalid marker: « {m} »",
            "marqueur invalide : « {m} »"
        )));
    }
    // end of the tag's line (after its remaining markers)
    let l2 = locate(&out, target)?;
    let end = l2.markers.last().map_or(l2.line.end, |s| s.end);
    let sep = if l2.markers.is_empty() { "  " } else { " " };
    Ok(splice(&out, end, end, &format!("{sep}@{m}")))
}

/// Sides of an arrow's ends (`side="start-end"`), empty = removed.
pub fn set_side(src: &str, edge: &str, side: &str) -> Result<String, EditError> {
    let t = Target::Edge(edge.to_string());
    if side.is_empty() || side == "-" {
        return remove_attr(src, &t, "side");
    }
    set_attr(src, &t, "side", side)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "//- commentaire\ndiagram(dir=\"down\")\n  card#fw.t0(tag=\"X\") Pare-feu   @1\n    li 10.0.0.1/24\n    to(core side=\"left-top\").wan transit\n  card#core.t1 Switch\n";

    /// Only the tag's line changes.
    fn one_line_changed(a: &str, b: &str) -> usize {
        a.lines().zip(b.lines()).filter(|(x, y)| x != y).count()
            + a.lines().count().abs_diff(b.lines().count())
    }

    #[test]
    fn appear_marker_set_replace_remove() {
        let src = "diagram\n  card#a A   @1:up @4:focus\n    to(b) go\n  card#b B\n";
        let t = Target::Node("a".into());
        let two = set_appear(src, &t, Some("2-3")).unwrap();
        assert!(two.contains("card#a A @4:focus @2-3"), "{two}");
        assert!(
            parse(&two).diagnostics.iter().all(|d| !d.is_error()),
            "{two}"
        );
        let none = set_appear(src, &t, None).unwrap();
        assert!(
            none.contains("card#a A @4:focus") && !none.contains("@1"),
            "{none}"
        );
        let edge = set_appear(src, &Target::Edge("a->b".into()), Some("2")).unwrap();
        assert!(edge.contains("to(b) go  @2"), "{edge}");
        let plain = set_appear(
            "diagram\n  card#b B\n",
            &Target::Node("b".into()),
            Some("1"),
        )
        .unwrap();
        assert_eq!(plain, "diagram\n  card#b B  @1\n");
        assert!(set_appear(src, &t, Some("a b")).is_err());
    }

    #[test]
    fn set_and_remove() {
        let out = move_card(SRC, "fw", 355.2, 150.0).unwrap();
        assert!(out.contains("card#fw.t0(tag=\"X\" at=\"355,150\") Pare-feu   @1"));
        assert_eq!(one_line_changed(SRC, &out), 1);
        let out2 = move_card(&out, "fw", 10.0, 20.0).unwrap();
        assert!(out2.contains("at=\"10,20\""));
        let back = release_card(&out2, "fw").unwrap();
        assert_eq!(back, SRC);
        let out = move_card(SRC, "core", 1.0, 2.0).unwrap();
        assert!(out.contains("card#core.t1(at=\"1,2\") Switch"));
        assert_eq!(release_card(&out, "core").unwrap(), SRC);
    }

    #[test]
    fn edges_and_classes() {
        let out = set_via(SRC, "fw->core", &[(300.0, 420.0)]).unwrap();
        assert!(out.contains("to(core side=\"left-top\" via=\"300,420\").wan transit"));
        let out = set_side(&out, "fw->core", "").unwrap();
        assert!(out.contains("to(core via=\"300,420\").wan transit"));
        let out = add_class(SRC, &Target::Edge("fw->core".into()), "dashed").unwrap();
        assert!(out.contains(".wan.dashed transit"));
        assert_eq!(
            remove_class(&out, &Target::Edge("fw->core".into()), "dashed").unwrap(),
            SRC
        );
        let plain = "card#a A\n  to(b) x\ncard#b B\n";
        let out = add_class(plain, &Target::Edge("a->b".into()), "dashed").unwrap();
        assert!(out.contains("to(b).dashed x"));
        assert_eq!(
            remove_class(&out, &Target::Edge("a->b".into()), "dashed").unwrap(),
            plain
        );
        let out = add_class(SRC, &Target::Node("core".into()), "secure").unwrap();
        assert!(out.contains("card#core.t1.secure Switch"));
        assert!(matches!(
            move_card(SRC, "absent", 1.0, 1.0),
            Err(EditError::NotFound(_))
        ));
    }

    #[test]
    fn icon_and_href_round_trip() {
        let t = Target::Node("fw".into());
        let out = set_attr(SRC, &t, "icon", "shield").unwrap();
        assert!(out.contains("icon=\"shield\""), "{out}");
        assert_eq!(one_line_changed(SRC, &out), 1);
        let out = set_attr(&out, &t, "href", "https://wiki.corp/fw").unwrap();
        assert!(
            parse(&out).model.nodes[0].href.is_some()
                || parse(&out).model.node("fw").unwrap().href.is_some()
        );
        assert_eq!(one_line_changed(SRC, &out), 1);
        let back = remove_attr(&remove_attr(&out, &t, "href").unwrap(), &t, "icon").unwrap();
        assert_eq!(back, SRC);
        let e = Target::Edge("fw->core".into());
        let out = set_attr(SRC, &e, "href", "#core").unwrap();
        assert_eq!(parse(&out).model.edges[0].href.as_deref(), Some("#core"));
        assert_eq!(remove_attr(&out, &e, "href").unwrap(), SRC);
    }

    #[test]
    fn diagram_options() {
        let out = set_attr(SRC, &Target::Diagram, "theme", "dark").unwrap();
        assert!(out.contains("diagram(dir=\"down\" theme=\"dark\")"));
    }
}
