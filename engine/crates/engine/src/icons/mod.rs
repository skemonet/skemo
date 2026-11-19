//! Built-in icon set: Lucide (ISC), 24 × 24, stroke 2, round caps (FORMAT §3.2.1), and skemo's
//! own `skemo-*` icons drawn the same way (D-115). Lucide's data is generated from its archive;
//! rendering is offline and deterministic.

mod data;
mod skemo;

use std::sync::OnceLock;

/// Both sets, sorted by name.
fn all() -> &'static [(&'static str, &'static str, &'static str)] {
    static ALL: OnceLock<Vec<(&str, &str, &str)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v: Vec<_> = data::ICONS.iter().chain(&skemo::SKEMO).copied().collect();
        v.sort_by_key(|e| e.0);
        v
    })
}

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
    let (name, markup, tags) = all()[i];
    Icon {
        name,
        markup,
        tags,
        image: None,
    }
}

/// The icon called `name` (kebab-case), if it exists: by its name, or by a name Lucide gave it in
/// an earlier release (`triangle-alert` is `triangle-exclamation-point` since 1.73).
pub fn get(name: &str) -> Option<Icon> {
    let at = |n: &str| all().binary_search_by(|e| e.0.cmp(n)).ok();
    at(name)
        .or_else(|| {
            let i = data::ALIASES.binary_search_by(|e| e.0.cmp(name)).ok()?;
            at(data::ALIASES[i].1)
        })
        .map(icon_at)
}

/// Every icon name, sorted.
pub fn names() -> impl Iterator<Item = &'static str> {
    all().iter().map(|e| e.0)
}

/// Names matching `query` (case-insensitive), best first: exact, prefix, name substring, tag
/// match. Each group is sorted by name.
pub fn search(query: &str) -> Vec<&'static str> {
    let q = query.trim().to_lowercase().replace(' ', "-");
    if q.is_empty() {
        return names().collect();
    }
    let mut groups: [Vec<&'static str>; 4] = Default::default();
    for (name, _, tags) in all() {
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
        // Lucide 1.73: a new icon, and a renamed one under its former name
        assert!(get("graph-ring").is_some());
        let old = get("triangle-alert").unwrap();
        assert_eq!(old.name, "triangle-exclamation-point");
        assert!(data::ALIASES.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(data::ALIASES.iter().all(|(_, n)| get(n).is_some()));
    }

    #[test]
    fn skemo_icons_join_the_set() {
        let vm = get("skemo-vm").unwrap();
        assert!(vm.markup.contains("stroke-dasharray") && vm.tags.contains("virtual machine"));
        assert_eq!(names().filter(|n| n.starts_with("skemo-")).count(), 36);
        assert!(search("hypervisor").contains(&"skemo-hypervisor"));
        // drawn in the text's colour, as Lucide's: no colour of their own
        for (name, markup, _) in &skemo::SKEMO {
            assert!(
                name.starts_with("skemo-") && !markup.contains('#'),
                "{name}"
            );
            assert!(get(name).is_some(), "{name}");
        }
    }
}
