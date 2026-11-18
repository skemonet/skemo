//! Warnings of the engine: a stable code, the element concerned when there is one, a message in the
//! current language. The command line turns the element into a line of the source
//! (`--diagnostics json`, `cli/SPEC.md` §2.2).

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EngineWarning {
    /// Stable code (`unknown-icon`, `overlap`…), listed in `cli/SPEC.md` §2.2.
    pub code: &'static str,
    /// Card, zone, line (`a/li[2]`) or arrow (`a->b`) the warning is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    pub message: String,
}

impl EngineWarning {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        EngineWarning {
            code,
            element: None,
            message: message.into(),
        }
    }

    /// The same warning, about `element`.
    pub fn on(mut self, element: impl Into<String>) -> Self {
        self.element = Some(element.into());
        self
    }

    /// Does the message contain `s`? (tests, filters)
    pub fn contains(&self, s: &str) -> bool {
        self.message.contains(s)
    }
}

impl std::fmt::Display for EngineWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<&crate::anim::Warning> for EngineWarning {
    fn from(w: &crate::anim::Warning) -> Self {
        EngineWarning::new(w.code, w.message.clone()).on(w.element.clone())
    }
}
