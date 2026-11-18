//! The files a diagram makes skemo read (images, the stylesheets of `diagram(css=…)`) are read
//! inside a root folder only (`--root`, `cli/SPEC.md` §2.3): a relative name, no `..`, no link
//! that leads out, a regular file, a bounded size. The files of the command line (`--css`,
//! `--base-theme`, `--font`) are the user's and are read wherever they are.

use skemo_lang::tr;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Largest stylesheet a diagram includes.
pub const MAX_CSS_BYTES: usize = 1 << 20;

/// The root folder, resolved (links followed) once.
#[derive(Debug, Clone)]
pub struct Root {
    dir: PathBuf,
}

/// Why a file was not read: its stable code (`--diagnostics json`) and the message.
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    pub code: &'static str,
    pub message: String,
}

fn refused(code: &'static str, message: String) -> Refused {
    Refused { code, message }
}

impl Root {
    /// `--root`, otherwise the folder of the source file, otherwise (standard input) the current
    /// folder.
    pub fn new(given: Option<&Path>, file: &str) -> Result<Root, String> {
        let dir = match given {
            Some(d) => d.to_path_buf(),
            None if file == "-" => PathBuf::from("."),
            None => Path::new(file)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
        };
        let dir = std::fs::canonicalize(&dir).map_err(|e| {
            tr!(
                "root folder `{}`: {}",
                "dossier racine « {} » : {}",
                dir.display(),
                crate::common::io(&e)
            )
        })?;
        if !dir.is_dir() {
            return Err(tr!(
                "root `{}`: not a folder",
                "racine « {} » : ce n'est pas un dossier",
                dir.display()
            ));
        }
        Ok(Root { dir })
    }

    /// Reads `name` (as written in the diagram) inside the root, at most `limit` bytes. `what`
    /// names the kind of file in the messages (« image », « stylesheet »).
    pub fn read(&self, name: &str, what: &str, limit: usize) -> Result<Vec<u8>, Refused> {
        let abs = name.starts_with(['/', '\\']) || name.as_bytes().get(1) == Some(&b':');
        if name.contains("://") || name.starts_with("data:") {
            return Err(refused(
                "file-remote",
                tr!(
                    "{what} `{name}`: only local files are accepted (rendering stays offline)",
                    "{what} « {name} » : seuls les fichiers locaux sont acceptés (le rendu reste hors ligne)"
                ),
            ));
        }
        if abs || name.split(['/', '\\']).any(|p| p == "..") || name.contains('\0') {
            return Err(refused(
                "file-outside-root",
                tr!(
                    "{what} `{name}`: a path relative to the root folder, inside it, is expected",
                    "{what} « {name} » : un chemin relatif au dossier racine, à l'intérieur, est attendu"
                ),
            ));
        }
        let path = self.dir.join(name);
        let real = std::fs::canonicalize(&path).map_err(|e| {
            refused(
                "file-missing",
                tr!(
                    "{what} `{name}`: {}",
                    "{what} « {name} » : {}",
                    crate::common::io(&e)
                ),
            )
        })?;
        if !real.starts_with(&self.dir) {
            return Err(refused(
                "file-outside-root",
                tr!(
                    "{what} `{name}`: a link that leads out of the root folder",
                    "{what} « {name} » : un lien qui sort du dossier racine"
                ),
            ));
        }
        let meta = std::fs::metadata(&real).map_err(|e| {
            refused(
                "file-missing",
                tr!(
                    "{what} `{name}`: {}",
                    "{what} « {name} » : {}",
                    crate::common::io(&e)
                ),
            )
        })?;
        if !meta.is_file() {
            return Err(refused(
                "file-special",
                tr!(
                    "{what} `{name}`: not a regular file",
                    "{what} « {name} » : ce n'est pas un fichier ordinaire"
                ),
            ));
        }
        let too_large = || {
            refused(
                "file-too-large",
                tr!(
                    "{what} `{name}`: larger than {} KiB",
                    "{what} « {name} » : plus de {} Kio",
                    limit / 1024
                ),
            )
        };
        if meta.len() > limit as u64 {
            return Err(too_large());
        }
        let mut data = Vec::new();
        std::fs::File::open(&real)
            .and_then(|f| f.take(limit as u64 + 1).read_to_end(&mut data))
            .map_err(|e| {
                refused(
                    "file-missing",
                    tr!(
                        "{what} `{name}`: {}",
                        "{what} « {name} » : {}",
                        crate::common::io(&e)
                    ),
                )
            })?;
        if data.len() > limit {
            return Err(too_large());
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("skemo-root-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("in/img")).unwrap();
        d
    }

    #[test]
    fn only_regular_files_inside_the_root_are_read() {
        let d = scratch("read");
        std::fs::write(d.join("in/img/a.svg"), "<svg/>").unwrap();
        std::fs::write(d.join("secret.svg"), "<svg/>").unwrap();
        std::fs::write(d.join("in/big.png"), vec![0u8; 2048]).unwrap();
        let root = Root::new(Some(&d.join("in")), "-").unwrap();
        assert_eq!(root.read("img/a.svg", "image", 1024).unwrap(), b"<svg/>");
        let code = |n: &str| root.read(n, "image", 1024).unwrap_err().code;
        assert_eq!(code("../secret.svg"), "file-outside-root");
        assert_eq!(code("/etc/passwd"), "file-outside-root");
        assert_eq!(code("https://x/y.svg"), "file-remote");
        assert_eq!(code("none.svg"), "file-missing");
        assert_eq!(code("big.png"), "file-too-large");
        assert_eq!(code("img"), "file-special");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(d.join("secret.svg"), d.join("in/link.svg")).unwrap();
            assert_eq!(code("link.svg"), "file-outside-root");
            std::os::unix::fs::symlink(d.join("in/img/a.svg"), d.join("in/ok.svg")).unwrap();
            assert!(root.read("ok.svg", "image", 1024).is_ok());
        }
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_default_root_is_the_source_folder() {
        let d = scratch("default");
        let f = d.join("in/a.skemo");
        let root = Root::new(None, f.to_str().unwrap()).unwrap();
        assert_eq!(root.dir, std::fs::canonicalize(d.join("in")).unwrap());
        assert!(Root::new(Some(&d.join("nope")), "-").is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
