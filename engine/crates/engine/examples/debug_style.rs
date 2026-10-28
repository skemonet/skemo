fn main() {
    let src = std::fs::read_to_string("fixtures/reseau-entete.pug").unwrap();
    let doc = skemo_lang::parse(&src);
    let scene = skemo_engine::scene::Scene::build(&doc.model, &[], skemo_lang::style::Theme::Light);
    let c = &scene.cards["fw"];
    let t = scene.style(c.title);
    let mut s = skemo_engine::measure::text_style(t);
    for w in [600u16, 700, 800] {
        s.weight = w;
        println!("{w} {}", s.width("Pare-feu périmétrique"));
    }
}
