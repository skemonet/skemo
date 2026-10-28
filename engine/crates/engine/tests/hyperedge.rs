//! Hyperedges (`join="trunk"`, D-034): the arrows of one `to(a b c …)` leave from the same point and
//! follow a common trunk before splitting.

use skemo_engine::{Options, from_source};

const SRC: &str = include_str!("data/hyperedge.skemo");

#[test]
fn arrows_of_one_to_share_their_start_and_first_leg() {
    let (d, _) = from_source(SRC, &Options::default()).unwrap();
    let starts: Vec<_> = ["ceo->cto", "ceo->cfo", "ceo->coo"]
        .iter()
        .map(|id| d.routes[*id][0])
        .collect();
    assert!(
        starts.windows(2).all(|w| w[0] == w[1]),
        "same attachment: {starts:?}"
    );
    // the trunk is vertical, on the same line for all three
    for id in ["ceo->cto", "ceo->cfo", "ceo->coo"] {
        let r = &d.routes[id];
        assert_eq!(r[1].0, r[0].0, "first leg vertical for {id}");
    }
}

#[test]
fn without_join_arrows_to_one_row_fork_by_themselves() {
    // D-048: the three executives share a row (ranks from the sources), so the plain arrows form
    // an automatic fork (D-047): one start, as with `join="trunk"`
    let plain = SRC.replace(" join=\"trunk\"", "");
    let (d, _) = from_source(&plain, &Options::default()).unwrap();
    let mut xs: Vec<f64> = ["ceo->cto", "ceo->cfo", "ceo->coo"]
        .iter()
        .map(|id| d.routes[*id][0].0)
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    assert_eq!(xs.len(), 1, "one start: an automatic fork");
}

#[test]
fn join_does_not_move_any_card() {
    let plain = SRC.replace(" join=\"trunk\"", "");
    let (a, _) = from_source(SRC, &Options::default()).unwrap();
    let (b, _) = from_source(&plain, &Options::default()).unwrap();
    assert_eq!(
        a.boxes, b.boxes,
        "the placement of the cards does not depend on join"
    );
}
