//! `side=` on an arrow: the side it leaves its source by and reaches its target by, named as
//! top, bottom, left, right or as a compass point (n, s, w, e), as `at=` names a band.

#[test]
fn compass_points_name_the_sides() {
    let doc = skemo_lang::parse(
        "diagram\n  card#a A\n    to(b side=\"n-s\")\n    to(b side=\"-e\")\n    to(b side=\"w\")\n  card#b B\n",
    );
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let sides: Vec<Option<&str>> = doc.model.edges.iter().map(|e| e.side.as_deref()).collect();
    assert_eq!(sides, [Some("top-bottom"), Some("-right"), Some("left")]);
}

#[test]
fn a_corner_is_not_a_side() {
    let doc = skemo_lang::parse("diagram\n  card#a A\n    to(b side=\"nw\")\n  card#b B\n");
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["bad-side"]);
    assert_eq!(doc.model.edges[0].side, None);
}

/// `at=` names a band by its sides too: `top` n, `bottom-right` se, `left-top` nw.
#[test]
fn sides_name_the_bands() {
    let doc = skemo_lang::parse(
        "diagram\n  card#a(at=\"top\") A\n  card#b(at=\"bottom-right\") B\n  card#c(at=\"left-top\") C\n  card#d(at=\"w\") D\n  card#e E\n",
    );
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let at: Vec<Option<&str>> = ["a", "b", "c", "d"]
        .iter()
        .map(|i| doc.model.node(i).unwrap().dock.as_deref())
        .collect();
    assert_eq!(at, [Some("n"), Some("se"), Some("nw"), Some("w")]);
}

#[test]
fn two_opposite_sides_are_no_band() {
    let doc = skemo_lang::parse("diagram\n  card#a(at=\"top-bottom\") A\n  card#e E\n");
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["bad-at"]);
}

/// `block`, `dir` and `align` (D-117); `at=` inside a zone or a block, not in a card.
#[test]
fn block_dir_align_and_docking_inside() {
    let doc = skemo_lang::parse(
        "diagram\n  zone#z(dir=\"horizontal\" align=\"center\") Z\n    card#a(at=\"top\") A\n    block#b(dir=\"vertical\")\n      card#c C\n",
    );
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let m = &doc.model;
    let z = m.node("z").unwrap();
    assert_eq!(
        (z.dir.as_deref(), z.align.as_deref()),
        (Some("right"), Some("center"))
    );
    assert_eq!(m.node("a").unwrap().dock.as_deref(), Some("n"));
    let b = m.node("b").unwrap();
    assert!(b.block && b.dir.as_deref() == Some("down"));
}
