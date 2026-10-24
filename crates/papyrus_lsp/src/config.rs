//! Server settings, sent by the client as `initializationOptions`, with
//! `workspace/didChangeConfiguration` or with a `papyrus/restart` request.

use std::path::PathBuf;

use serde::Deserialize;

/// The `papyrus` settings section.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Config {
    /// Extra directories with script sources, e.g. the game's
    /// `Data/Source/Scripts` and the sources of required mods. Earlier
    /// directories win when several declare the same script.
    pub(crate) import_directories: Vec<PathBuf>,
    /// Report unknown types, names and members.
    pub(crate) semantic_diagnostics: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config { import_directories: Vec::new(), semantic_diagnostics: true }
    }
}

impl Config {
    /// Reads settings from a JSON value, accepting both the bare section and an
    /// object wrapping it under a `papyrus` key. Invalid values are logged and
    /// replaced by defaults.
    pub(crate) fn from_json(value: Option<serde_json::Value>) -> Config {
        let Some(mut value) = value.filter(|v| !v.is_null()) else {
            return Config::default();
        };
        if let Some(section) = value.get_mut("papyrus") {
            value = section.take();
        }
        match serde_json::from_value(value) {
            Ok(config) => config,
            Err(err) => {
                log::warn!("invalid settings, using defaults: {err}");
                Config::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wrapped_and_bare_sections() {
        let wrapped = serde_json::json!({ "papyrus": { "importDirectories": ["a"] } });
        assert_eq!(Config::from_json(Some(wrapped)).import_directories, [PathBuf::from("a")]);
        let bare = serde_json::json!({ "semanticDiagnostics": false });
        assert!(!Config::from_json(Some(bare)).semantic_diagnostics);
        assert_eq!(Config::from_json(None), Config::default());
    }
}
