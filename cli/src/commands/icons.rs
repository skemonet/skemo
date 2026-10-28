//! `skemo icons [query]`: the built-in icons (FORMAT §3.2.1), one per line or as JSON.

use crate::common::{Fail, write_out};
use skemo_engine::icons;

pub fn run(query: Option<&str>, json: bool) -> Result<(), Fail> {
    let names = icons::search(query.unwrap_or_default());
    let text = if json {
        let rows: Vec<serde_json::Value> = names
            .iter()
            .filter_map(|n| icons::get(n))
            .map(|i| {
                let tags: Vec<&str> = i.tags.split(' ').filter(|t| !t.is_empty()).collect();
                serde_json::json!({ "name": i.name, "tags": tags })
            })
            .collect();
        serde_json::to_string_pretty(&rows).unwrap_or_default() + "\n"
    } else {
        names.iter().map(|n| format!("{n}\n")).collect()
    };
    write_out("-", text.as_bytes())
}
