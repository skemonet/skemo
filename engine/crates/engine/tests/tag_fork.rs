//! A fork whose source's axis runs just past a zone's tag pill (D-057): the trunk goes straight
//! up to the split, at the level of its neighbour's fork, instead of stepping round the pill.

use skemo_engine::{Options, from_source};

#[test]
fn a_fork_beside_a_zone_tag_splits_on_its_axis_like_its_neighbour() {
    let src = include_str!("data/service-types.skemo");
    let css = include_str!("data/service-types.css");
    let opts = Options {
        css: vec![css.to_string()],
        ..Options::default()
    };
    let d = from_source(src, &opts).unwrap().0;
    let front = &d.routes["s-front->f1"];
    let back = &d.routes["s-back->b1"];
    // out of the service, straight up to the bar
    assert!((front[0].0 - front[1].0).abs() < 0.5, "{front:?}");
    // the two forks split at one level
    assert!((front[1].1 - back[1].1).abs() < 0.5, "{front:?} {back:?}");
}
