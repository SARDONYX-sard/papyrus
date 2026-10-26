//! Server settings, sent by the client as `initializationOptions`, with
//! `workspace/didChangeConfiguration` or with a `papyrus/restart` request.

use std::{collections::BTreeMap, path::PathBuf};

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
    /// The flags file, when `papyrus.toml` does not set one.
    pub(crate) flags_file: Option<PathBuf>,
    /// Show parameter names before call arguments.
    pub(crate) inlay_hints: InlayHintsConfig,
    /// Values of `${NAME}` variables, e.g. `SKYRIM_DIR`. They win over `[var]`
    /// of `papyrus.local.toml` and `papyrus.toml`.
    pub(crate) var: BTreeMap<String, String>,
}

/// The `inlayHints` settings.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct InlayHintsConfig {
    /// Parameter names before call arguments.
    pub(crate) parameter_names: bool,
    /// `overrides Quest` after functions replacing one of a parent script.
    pub(crate) overrides: bool,
}

impl Default for InlayHintsConfig {
    fn default() -> Self {
        InlayHintsConfig { parameter_names: true, overrides: true }
    }
}

/// Options given on the command line, for editors that cannot send settings
/// easily. Settings sent by the client are applied on top of them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CliOptions {
    pub(crate) var: BTreeMap<String, String>,
    pub(crate) import_directories: Vec<PathBuf>,
    pub(crate) flags_file: Option<PathBuf>,
}

/// Usage of the command line.
pub(crate) const USAGE: &str = "\
papyrus_analyzer: language server for the Papyrus scripting language (stdio)

USAGE:
    papyrus_analyzer [OPTIONS]            run the language server on stdio
    papyrus_analyzer <COMMAND> [ARGS]...

COMMANDS:
    compile      compile the project of papyrus.toml, or like PapyrusCompiler.exe
    decompile    decompile .pex files to .psc
    disasm       print the disassembly of .pex files
    fmt          format scripts
    lint         check scripts for problems, and fix them
    test         run the `; #[test]` functions of the scripts
    run          run a function in the Papyrus VM, natives mocked
    help         print this help, or the help of a command

`papyrus_analyzer <COMMAND> --help` (or `help <COMMAND>`) prints the options of a command.

OPTIONS (language server):
    --var NAME=VALUE      Value of a `${NAME}` variable of papyrus.toml, e.g.
                          --var SKYRIM_DIR=\"D:/Steam/steamapps/common/Skyrim Special Edition\"
                          (`${env:NAME}` reads environment variables instead)
    --import-dir PATH     Extra script source directory (repeatable)
    --flags-file PATH     The flags file when papyrus.toml does not set one
    -V, --version         Print the version
    -h, --help            Print this help

Logs go to stderr; set RUST_LOG=debug for more.";

impl CliOptions {
    /// Parses the arguments after the program name.
    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<CliOptions, String> {
        let mut out = CliOptions::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            // Accept both `--flag value` and `--flag=value`.
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
                _ => (arg.clone(), None),
            };
            let mut value = || {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .ok_or_else(|| format!("`{flag}` needs a value"))
            };
            match flag.as_str() {
                "--var" => {
                    let pair = value()?;
                    let (name, v) = pair
                        .split_once('=')
                        .ok_or_else(|| format!("`--var {pair}`: expected NAME=VALUE"))?;
                    out.var.insert(name.to_owned(), v.to_owned());
                }
                "--import-dir" => out.import_directories.push(PathBuf::from(value()?)),
                "--flags-file" => out.flags_file = Some(PathBuf::from(value()?)),
                other => return Err(format!("unknown argument `{other}`\n\n{USAGE}")),
            }
        }
        Ok(out)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            import_directories: Vec::new(),
            semantic_diagnostics: true,
            flags_file: None,
            var: BTreeMap::new(),
            inlay_hints: InlayHintsConfig::default(),
        }
    }
}

impl Config {
    /// Reads settings from a JSON value, accepting both the bare section and an
    /// object wrapping it under a `papyrus` key. Invalid values are logged and
    /// replaced by defaults.
    /// Applies command line options below these settings.
    pub(crate) fn with_cli(mut self, cli: &CliOptions) -> Config {
        for (name, value) in &cli.var {
            self.var.entry(name.clone()).or_insert_with(|| value.clone());
        }
        let mut dirs = cli.import_directories.clone();
        dirs.extend(
            self.import_directories.drain(..).filter(|d| !cli.import_directories.contains(d)),
        );
        self.import_directories = dirs;
        if self.flags_file.is_none() {
            self.flags_file = cli.flags_file.clone();
        }
        self
    }

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

    #[test]
    fn cli_options() {
        let args = ["--var", "SKYRIM_DIR=D:/Sky rim", "--import-dir=a", "--flags-file", "f.flg"];
        let cli = CliOptions::parse(args.iter().map(|s| (*s).to_owned())).unwrap();
        assert_eq!(cli.var["SKYRIM_DIR"], "D:/Sky rim");
        assert_eq!(cli.import_directories, [PathBuf::from("a")]);

        let settings =
            serde_json::json!({ "var": { "SKYRIM_DIR": "E:/x" }, "importDirectories": ["b"] });
        let config = Config::from_json(Some(settings)).with_cli(&cli);
        assert_eq!(config.var["SKYRIM_DIR"], "E:/x", "settings win over arguments");
        assert_eq!(config.import_directories, [PathBuf::from("a"), PathBuf::from("b")]);
        assert_eq!(config.flags_file, Some(PathBuf::from("f.flg")));

        assert!(CliOptions::parse(["--nope".to_owned()]).is_err());
        assert!(CliOptions::parse(["--var".to_owned(), "X".to_owned()]).is_err());
    }
}
