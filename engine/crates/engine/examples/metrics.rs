//! Vertical metrics of each embedded font (hhea, typo, win), to tune the measurement.
fn main() {
    for f in &skemo_engine::fonts::FILES {
        let font = harfrust::Font::new(f.data, 0).expect("police");
        let m = font.metrics();
        println!(
            "{} {} upem={} hhea={:?} typo={:?} win={:?} use_typo={}",
            f.file,
            f.weight,
            m.units_per_em,
            m.hhea_line,
            m.typo_line,
            m.win_line,
            m.use_typo_metrics
        );
    }
}
