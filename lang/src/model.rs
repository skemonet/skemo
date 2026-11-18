//! Diagram model (SPEC §2), serialisable to JSON compatible with the prototype's output
//! (`fixtures/*.model.json`). The fields the Rust version adds (`anim`, `li`) come on top; no field
//! of the prototype is renamed.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

/// A step value, as written (`"2.5"`) and as a number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub raw: String,
    pub value: f64,
}

/// Plage de mise en valeur (`@4-6:focus`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    pub from: Step,
    pub to: Option<Step>,
}

/// Highlight range (`@3-4:halo`, FORMAT §3.8): an effect the integration plays on the element
/// during those steps (`halo`, `chase`, `magnify`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    pub from: Step,
    pub to: Option<Step>,
    pub effect: String,
}

/// Animation markers of an element (FORMAT §3.8), as written (not resolved).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Anim {
    pub start: Option<Step>,
    pub until: Option<Step>,
    pub fx: Option<String>,
    pub focus: Vec<Focus>,
    /// Highlights in writing order. Absent from the JSON when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub highlights: Vec<Highlight>,
}

impl Anim {
    pub fn is_empty(&self) -> bool {
        self.start.is_none()
            && self.until.is_none()
            && self.fx.is_none()
            && self.focus.is_empty()
            && self.highlights.is_empty()
    }
}

/// A line `|` of a card or a zone (FORMAT §3.2.3), or a table row (§3.2.5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Li {
    /// The text as written, emphasis delimiters included (`inline::runs` resolves them);
    /// markers removed. For a row, the row as written after the opening bar.
    pub text: String,
    pub anim: Option<Anim>,
    /// `|.pill text` (D-050): classes of the line, styled in CSS. Absent from the JSON when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// A table row: its cells (`table::blocks` groups rows into tables). Absent when not a row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Cell>,
    /// A separator row (`|---|:-:|`): the alignment of each column. Absent otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule: Vec<Align>,
}

/// A cell of a table row, as written.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Cell {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// `||` (nothing in the cell) joins the cell on the left; `^^` the cell above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<Join>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    Left,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Card,
    Zone,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: Kind,
    pub parent: Option<String>,
    pub classes: Vec<String>,
    pub title: String,
    pub tag: Option<String>,
    /// `label="…"` (FORMAT §3.2.6): the card's category, overriding the type's `--label`;
    /// `Some("")` shows none. Absent from the JSON when not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// `at="s"` (FORMAT §3.2.9): a compass point (`n s e w ne nw se sw`); the element is docked
    /// outside the drawing. Absent from the JSON when not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dock: Option<String>,
    /// Step markers written in the `tag` value (`tag="8080 @1"`): the chip appears on its own.
    /// Absent from the JSON when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_anim: Option<Anim>,
    /// Texts of the `li` lines (prototype compatibility); the detail is in `li`.
    pub items: Vec<String>,
    pub at: Option<[f64; 2]>,
    pub size: Option<[f64; 2]>,
    /// Appearance step (prototype compatibility: a number). See `anim` for the full form.
    pub step: Option<Number>,
    pub fx: Option<String>,
    pub date: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub anim: Option<Anim>,
    pub li: Vec<Li>,
    /// `icon="name"` (FORMAT §3.2.1). Absent from the JSON when not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// `href="…"` (FORMAT §3.2.1), already checked. Absent from the JSON when not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// `wrap="N"` on a zone: at most N children per row (per column with `dir` right or left).
    /// Absent from the JSON when it is not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<u32>,
    /// `stack` (D-050): an invisible zone whose children are laid one after the other in writing
    /// order along `dir`. Absent from the JSON when false.
    #[serde(default, skip_serializing_if = "is_no")]
    pub stack: bool,
    /// `cards`: a card drawn as a pile of cards (several instances of one thing); `count`, when
    /// written, is shown as a `×N` badge. Absent from the JSON for a plain card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deck: Option<Deck>,
    /// `node` (D-087): a card drawn as its icon, with its name and lines beside it and no frame;
    /// the looks do not apply to it. Absent from the JSON when false.
    #[serde(default, skip_serializing_if = "is_no")]
    pub node: bool,
}

/// A pile of cards (`cards`): how many instances it stands for, if known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Deck {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
}

impl Deck {
    /// Cards drawn: the count, at most 3; 3 when the count is not written.
    pub fn layers(&self) -> u32 {
        self.count.map_or(3, |n| n.clamp(1, 3))
    }
}

/// The diagram's title (`title` under `diagram`, D-050): its text, its pills (`|.pill`) and its
/// subtitle lines (`|`, v0.8).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Title {
    pub text: String,
    /// Pills on the title row, as drawn (a link's text without its Markdown).
    pub lines: Vec<String>,
    /// The classes of each pill besides `pill` (`|.pill.ocp text`: a colour, an icon, styled in
    /// CSS), when one has some. Absent from the JSON when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pill_classes: Vec<Vec<String>>,
    /// The link of each pill written as a Markdown link (`|.pill [text](other.skemo)`), already
    /// checked, when one has some. Absent from the JSON when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pill_hrefs: Vec<Option<String>>,
    /// Subtitle lines under the title, emphasis included. Absent from the JSON when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub: Vec<String>,
    /// The classes of each subtitle line (`|.warn text`), when one has some. Absent from the JSON
    /// when none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sub_classes: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub classes: Vec<String>,
    pub label: String,
    pub via: Option<Vec<[f64; 2]>>,
    pub side: Option<String>,
    /// `join="trunk"`: the arrows leaving one card from one `to(a b c)` share a trunk (hyperedge).
    /// Absent from the JSON when not written (compatibility with the oracle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    /// `constraint="false"`: the arrow is drawn and routed but plays no part in the automatic
    /// layout (Graphviz's `constraint=false`). Left out of the JSON when true (the default).
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub constraint: bool,
    /// `href="…"` on an arrow, already checked. Absent from the JSON when not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    pub step: Option<Number>,
    pub fx: Option<String>,
    pub anim: Option<Anim>,
}

fn yes() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
fn is_no(b: &bool) -> bool {
    !*b
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
fn is_yes(b: &bool) -> bool {
    *b
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Model {
    /// Attributes of `diagram(...)`, as written (strings), as in the prototype.
    pub options: Map<String, Value>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// `title` (D-050). Absent from the JSON when not written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Title>,
}

impl Model {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Option `diagram(nom=…)` sous forme de texte.
    pub fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).and_then(Value::as_str)
    }

    /// Direct children of `parent` (`None` = root), in writing order.
    pub fn children<'a>(&'a self, parent: Option<&'a str>) -> impl Iterator<Item = &'a Node> + 'a {
        self.nodes
            .iter()
            .filter(move |n| n.parent.as_deref() == parent)
    }

    /// Ancestor zones of `id`, from the nearest to the furthest.
    pub fn ancestors(&self, id: &str) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut cur = self.node(id).and_then(|n| n.parent.as_deref());
        while let Some(p) = cur {
            match self.node(p) {
                Some(n) if !out.iter().any(|o: &&Node| o.id == n.id) => {
                    out.push(n);
                    cur = n.parent.as_deref();
                }
                _ => break,
            }
        }
        out
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// JSON number "as the prototype writes it": an integer if the value is whole.
pub(crate) fn number(v: f64) -> Number {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        Number::from(v as i64)
    } else {
        Number::from_f64(v).unwrap_or_else(|| Number::from(0))
    }
}

/// A link to another diagram (D-097): an `href` whose path, before any `#`, is a relative path to a
/// `.skemo` file (`network.skemo`, `../shop/api.skemo#db`). Returns the path and the fragment.
/// `skemo site` turns such links into links between the pages it writes; every output draws them
/// with a zoom glyph rather than the external-link one.
///
/// ```
/// use skemo_lang::model::diagram_link;
/// assert_eq!(diagram_link("api.skemo#db"), Some(("api.skemo", Some("db"))));
/// assert_eq!(diagram_link("../a/B.SKEMO"), Some(("../a/B.SKEMO", None)));
/// assert_eq!(diagram_link("https://x.org/a.skemo"), None);
/// assert_eq!(diagram_link("/abs/a.skemo"), None);
/// assert_eq!(diagram_link("notes.md"), None);
/// ```
pub fn diagram_link(href: &str) -> Option<(&str, Option<&str>)> {
    let (path, fragment) = match href.split_once('#') {
        Some((p, f)) => (p, Some(f).filter(|f| !f.is_empty())),
        None => (href, None),
    };
    let relative = !path.starts_with(['/', '\\'])
        && !path.contains([':', '?'])
        && !path.chars().any(char::is_whitespace);
    let named = path.len() > ".skemo".len()
        && path
            .get(path.len() - ".skemo".len()..)
            .is_some_and(|e| e.eq_ignore_ascii_case(".skemo"))
        && !path.ends_with("/.skemo");
    (relative && named).then_some((path, fragment))
}
