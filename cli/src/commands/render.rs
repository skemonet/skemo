//! `skemo render`: HTML, SVG, PNG (single, per state, contact sheet), GIF and APNG.

use crate::args::{ContrastArg, Format, RenderArgs, ThemeArg};
use crate::common::{Fail, Inputs, prepare, with_source, write_out};
use crate::out::Out;
use skemo_engine::Diagram;
use skemo_engine::animated::{GifOptions, apng, gif, state_png, states_png};
use skemo_engine::html::HtmlOptions;
use skemo_engine::source::Bundle;
use skemo_lang::tr;
use std::path::Path;

pub fn run(out: &Out, a: &RenderArgs) -> Result<(), Fail> {
    let fmt = match a.format {
        Some(f) => f,
        None => format_of(&a.output)?,
    };
    check_flags(a, fmt)?;
    let inputs = Inputs::new(&a.file, &a.css, a.base_theme.as_ref())
        .theme(a.theme)
        .high_contrast(a.contrast == ContrastArg::High)
        .transparent(a.transparent)
        .font_scale(a.font_scale)
        .fonts(&a.fonts.font);
    let (d, warnings, bundle) = prepare(out, inputs)?;
    let src = (!a.no_source).then_some(&bundle);
    if a.theme == Some(ThemeArg::Auto) && fmt != Format::Html {
        out.warn(&tr!(
            "theme « auto »: rendered with the light theme outside HTML",
            "thème « auto » : rendu en thème clair hors HTML"
        ));
    }
    if a.transparent && matches!(fmt, Format::Html | Format::Gif) {
        out.warn(&tr!(
            "--transparent has no effect for HTML and GIF: the theme's background is kept",
            "--transparent est sans effet en HTML et en GIF : le fond du thème est conservé"
        ));
    }
    if a.deny_warnings && warnings > 0 {
        return Err(Fail(
            1,
            Some(tr!(
                "{warnings} warning(s) with --deny-warnings",
                "{warnings} avertissement(s) avec --deny-warnings"
            )),
        ));
    }
    match fmt {
        Format::Html => html(a, &d, src),
        Format::Svg => svg(out, a, &d, src),
        Format::Png => png(a, &d, src),
        Format::Gif | Format::Apng => animation(a, &d, fmt, src),
        Format::Webp => webp(a, &d, src),
    }
}

/// Format deduced from the extension of the output file.
fn format_of(output: &str) -> Result<Format, Fail> {
    let ext = Path::new(output)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("html" | "htm") => Ok(Format::Html),
        Some("svg") => Ok(Format::Svg),
        Some("png") => Ok(Format::Png),
        Some("gif") => Ok(Format::Gif),
        Some("apng") => Ok(Format::Apng),
        Some("webp") => Ok(Format::Webp),
        _ => Err(Fail::usage(tr!(
            "unknown format for « {output} »: use .html, .svg, .png, .gif, .apng or .webp, or --format",
            "format de « {output} » inconnu : utiliser .html, .svg, .png, .gif, .apng ou .webp, ou --format"
        ))),
    }
}

/// Combinations of options inconsistent with the format.
fn check_flags(a: &RenderArgs, fmt: Format) -> Result<(), Fail> {
    if !a.png.animated {
        return Ok(());
    }
    if fmt == Format::Svg {
        return Err(Fail::usage(tr!(
            "--animated is meaningless for SVG: the SVG is always marked (data-step…) and never animated by the engine",
            "--animated est sans objet en SVG : le SVG est toujours marqué (data-step…) et n'est jamais animé par le moteur"
        )));
    }
    if fmt != Format::Png {
        return Err(Fail::usage(tr!(
            "--animated only applies to PNG (one file per state); for an animation use .gif, .apng or .html",
            "--animated ne s'applique qu'au PNG (un fichier par état) ; pour une animation, utiliser .gif, .apng ou .html"
        )));
    }
    if a.output == "-" {
        return Err(Fail::usage(tr!(
            "--animated writes several files: give an output name",
            "--animated écrit plusieurs fichiers : donner un nom de sortie"
        )));
    }
    Ok(())
}

fn html(a: &RenderArgs, d: &Diagram, src: Option<&Bundle>) -> Result<(), Fail> {
    let title = Path::new(&a.file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("skemo")
        .to_string();
    let opts = HtmlOptions {
        title,
        embed_fonts: !a.fonts.no_fonts,
    };
    let page = skemo_engine::html::render(d, &opts).into_bytes();
    write_out(&a.output, &with_source(page, src))
}

fn svg(out: &Out, a: &RenderArgs, d: &Diagram, src: Option<&Bundle>) -> Result<(), Fail> {
    if a.png.step.is_some() {
        out.warn(&tr!(
            "--step is meaningless for SVG (the SVG holds every state, marked)",
            "--step est sans objet en SVG (le SVG contient tous les états, marqués)"
        ));
    }
    let svg = if a.fonts.embed_fonts {
        skemo_engine::svg::render_with_fonts(d)
    } else {
        skemo_engine::svg::render(d)
    };
    write_out(&a.output, &with_source(svg.into_bytes(), src))
}

/// PNG: contact sheet of the steps, one file per state, one given state, or the full diagram.
fn png(a: &RenderArgs, d: &Diagram, src: Option<&Bundle>) -> Result<(), Fail> {
    // contact sheet of the steps: scale 1 by default (as the prototype's); otherwise 2
    let scale = a.scale.unwrap_or(if a.png.board { 1.0 } else { 2.0 });
    if a.png.board {
        let bytes = skemo_engine::animated::board(d, scale).map_err(Fail::internal)?;
        write_out(&a.output, &with_source(bytes, src))
    } else if a.png.animated {
        let (pngs, index) = states_png(d, scale).map_err(Fail::internal)?;
        let base = a.output.strip_suffix(".png").unwrap_or(&a.output);
        for (k, p) in pngs.iter().enumerate() {
            write_out(&format!("{base}.{k:02}.png"), &with_source(p.clone(), src))?;
        }
        let json = serde_json::to_string_pretty(&index).unwrap_or_default();
        write_out(&format!("{base}.states.json"), json.as_bytes())
    } else if let Some(s) = &a.png.step {
        let bytes = state_png(d, s, scale).map_err(Fail::usage)?;
        write_out(&a.output, &with_source(bytes, src))
    } else {
        let bytes = skemo_engine::png::render(d, scale).map_err(Fail::internal)?;
        write_out(&a.output, &with_source(bytes, src))
    }
}

/// Lossless WebP: the state asked for (`--step`), the animation if the diagram has several states,
/// otherwise the still image.
fn webp(a: &RenderArgs, d: &Diagram, src: Option<&Bundle>) -> Result<(), Fail> {
    if let Some(s) = &a.png.step {
        let bytes = skemo_engine::webp::state(d, s, a.scale.unwrap_or(2.0)).map_err(Fail::usage)?;
        return write_out(&a.output, &with_source(bytes, src));
    }
    let bytes = if d.anim.states.len() > 1 {
        skemo_engine::webp::animated(d, &gif_options(a))
    } else {
        skemo_engine::webp::render(d, a.scale.unwrap_or(2.0))
    };
    write_out(&a.output, &with_source(bytes.map_err(Fail::internal)?, src))
}

/// Common settings of the animated outputs (GIF, APNG, WebP).
fn gif_options(a: &RenderArgs) -> GifOptions {
    GifOptions {
        hold: a.anim.hold,
        transition: a.anim.transition,
        draw_transition: a.anim.transition.max(0.6),
        fps: a.anim.fps.clamp(1, 50),
        looping: !a.anim.no_loop,
        final_hold: a.anim.final_hold,
        scale: a.scale.unwrap_or(1.0),
    }
}

fn animation(a: &RenderArgs, d: &Diagram, fmt: Format, src: Option<&Bundle>) -> Result<(), Fail> {
    let o = gif_options(a);
    let bytes = if fmt == Format::Gif {
        gif(d, &o)
    } else {
        apng(d, &o)
    };
    write_out(&a.output, &with_source(bytes.map_err(Fail::internal)?, src))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_follows_the_extension_case_insensitively() {
        assert_eq!(format_of("a.PNG").unwrap(), Format::Png);
        assert_eq!(format_of("x/y.htm").unwrap(), Format::Html);
        assert_eq!(format_of("a.apng").unwrap(), Format::Apng);
    }

    #[test]
    fn unknown_extension_is_a_usage_error() {
        let e = format_of("a.txt").unwrap_err();
        assert_eq!(e.0, 2);
        assert!(e.1.unwrap().contains("a.txt"));
        assert_eq!(format_of("-").unwrap_err().0, 2);
    }
}
