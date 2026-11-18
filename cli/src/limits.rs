//! Bounded work (`cli/SPEC.md` §2.6): a diagram larger than these limits is refused with a clear
//! error rather than laid out for minutes.

use skemo_lang::tr;

/// Largest source, in bytes.
pub const MAX_SOURCE_BYTES: usize = 512 * 1024;
/// Most cards and zones.
pub const MAX_NODES: usize = 1000;
/// Most arrows.
pub const MAX_ARROWS: usize = 1000;
/// Most lines (`|`) in all the cards.
pub const MAX_LINES: usize = 5000;

/// The limit a source passes, if any: its code and message.
pub fn check(src: &str) -> Result<(), String> {
    if src.len() > MAX_SOURCE_BYTES {
        return Err(tr!(
            "the source is larger than {} KiB",
            "la source dépasse {} Kio",
            MAX_SOURCE_BYTES / 1024
        ));
    }
    let model = skemo_lang::parse(src).model;
    let lines: usize = model.nodes.iter().map(|n| n.li.len()).sum();
    let over = |what: String, n: usize, max: usize| {
        (n > max).then(|| {
            tr!(
                "{n} {what}: at most {max} in a diagram",
                "{n} {what} : au plus {max} dans un schéma"
            )
        })
    };
    let found = over(
        tr!("cards and zones", "cartes et zones"),
        model.nodes.len(),
        MAX_NODES,
    )
    .or_else(|| over(tr!("arrows", "flèches"), model.edges.len(), MAX_ARROWS))
    .or_else(|| over(tr!("lines", "lignes"), lines, MAX_LINES));
    found.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_diagram_over_a_limit_is_refused() {
        assert!(check("diagram\n  card#a A\n").is_ok());
        let mut big = String::from("diagram\n");
        for k in 0..=MAX_NODES {
            big.push_str(&format!("  card#c{k} C\n"));
        }
        assert!(check(&big).unwrap_err().contains(&MAX_NODES.to_string()));
        assert!(check(&"x".repeat(MAX_SOURCE_BYTES + 1)).is_err());
    }
}
