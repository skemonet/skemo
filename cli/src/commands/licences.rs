//! `skemo licences`: the licences and copyright notices of what the binary embeds (the fonts,
//! the icons, the Rust crates compiled in), so that they travel with the binary alone.

use crate::common::{Fail, write_out};
use skemo_lang::tr;

/// The Rust crates compiled in: name, version, licence (`tools/gen-licences.py`).
pub const CRATES: &str = include_str!("../crates.txt");

pub fn run() -> Result<(), Fail> {
    write_out("-", text().as_bytes())
}

fn text() -> String {
    let rule = "=".repeat(78);
    let mut out = tr!(
        "skemo {} — MIT OR Apache-2.0, at your option (https://skemo.net).\n\nIt embeds the following, under their own licences.\n",
        "skemo {} — MIT OR Apache-2.0, au choix (https://skemo.net).\n\nIl embarque ce qui suit, sous leurs propres licences.\n",
        env!("CARGO_PKG_VERSION")
    );
    for (what, licence, notice) in skemo_engine::notices() {
        out.push_str(&format!(
            "\n{rule}\n{what} — {licence}\n{rule}\n\n{}\n",
            notice.trim_end()
        ));
    }
    out.push_str(&format!(
        "\n{rule}\n{}\n{rule}\n\n",
        tr!(
            "Rust crates (name, version, licence; texts in each crate's repository)",
            "Crates Rust (nom, version, licence ; textes dans le dépôt de chaque crate)"
        )
    ));
    out.push_str(CRATES);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn says_every_embedded_part() {
        let t = super::text();
        for part in [
            "Inter",
            "JetBrains Mono",
            "Lucide",
            "OFL",
            "ISC",
            "serde ",
            "EPL-2.0",
        ] {
            assert!(t.contains(part), "{part} missing");
        }
    }
}
