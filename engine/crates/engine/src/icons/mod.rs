//! Built-in icon set: Lucide (ISC), 24 × 24, stroke 2, round caps (FORMAT §3.2.1).
//! The data is generated (`tools/gen-icons.py`); rendering is offline and deterministic.

mod data;

use data::ICONS;

/// One icon of the set.
#[derive(Debug, Clone, Copy)]
pub struct Icon {
    pub name: &'static str,
    /// Inner SVG markup (`path`, `circle`, … elements) in a 24 × 24 box, to be placed in a group
    /// with `fill="none" stroke="currentColor" stroke-width="2"` and round caps and joins.
    pub markup: &'static str,
    /// Search tags, separated by spaces.
    pub tags: &'static str,
    /// An image file instead of a built-in drawing (D-046): its index in `Scene::images`; `name`,
    /// `markup` and `tags` are then empty.
    pub image: Option<usize>,
}

impl Icon {
    /// The image file at `index` of `Scene::images`.
    pub fn image(index: usize) -> Icon {
        Icon {
            name: "",
            markup: "",
            tags: "",
            image: Some(index),
        }
    }
}

fn icon_at(i: usize) -> Icon {
    let (name, markup, tags) = ICONS[i];
    Icon {
        name,
        markup,
        tags,
        image: None,
    }
}

/// The icon called `name` (kebab-case), if it exists.
pub fn get(name: &str) -> Option<Icon> {
    ICONS.binary_search_by(|e| e.0.cmp(name)).ok().map(icon_at)
}

/// Every icon name, sorted.
pub fn names() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|e| e.0)
}

/// Names matching `query` (case-insensitive), best first: exact, prefix, name substring, tag
/// match. Each group is sorted by name.
pub fn search(query: &str) -> Vec<&'static str> {
    let q = query.trim().to_lowercase().replace(' ', "-");
    if q.is_empty() {
        return names().collect();
    }
    let mut groups: [Vec<&'static str>; 4] = Default::default();
    for (name, _, tags) in &ICONS {
        let rank = if *name == q {
            0
        } else if name.starts_with(&q) {
            1
        } else if name.contains(&q) {
            2
        } else if tags.split(' ').any(|t| t.to_lowercase().contains(&q)) {
            3
        } else {
            continue;
        };
        groups[rank].push(name);
    }
    groups.into_iter().flatten().collect()
}

/// The closest known name to a misspelled `name`, for the `unknown-icon` suggestion.
pub fn suggest(name: &str) -> Option<&'static str> {
    skemo_lang::diag::suggest(name, names()).or_else(|| search(name).first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_and_search() {
        assert!(names().count() > 1500);
        let sorted: Vec<_> = names().collect();
        assert!(sorted.windows(2).all(|w| w[0] < w[1]));
        let db = get("database").unwrap();
        assert!(db.markup.contains("<ellipse"));
        assert!(get("nope").is_none());
        assert_eq!(search("database")[0], "database");
        assert!(search("servers").contains(&"database"), "tag match");
        assert_eq!(suggest("databse"), Some("database"));
    }
}
