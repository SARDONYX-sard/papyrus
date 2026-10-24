//! `${NAME}` variables and `${env:NAME}` environment variables.
//!
//! `papyrus.toml` is shared through version control, so machine specific
//! paths do not belong in it. They are written as variables instead, and each
//! machine provides the values. `${NAME}` is looked up in, first wins:
//!
//! 1. the editor: the `papyrus.var` setting, or `papyrus_analyzer --var NAME=VALUE`;
//! 2. `[var]` of `papyrus.local.toml`, a file next to the manifest that is not
//!    committed;
//! 3. `[var]` of `papyrus.toml`, shared defaults.
//!
//! An undefined variable is an error: nothing is guessed. Like in VS Code
//! settings, `${env:NAME}` reads the environment variable `NAME`, and
//! `${workspaceFolder}` (or `${workspaceFolder:name}` with several folders) is
//! a workspace folder. `${game:SkyrimSE}` is the installation directory of a
//! game found in the registry (`SkyrimLE`, `SkyrimSE`, `SkyrimVR`, `Enderal`,
//! `EnderalSE`); it is read from the Windows registry and is therefore only
//! available on Windows. Variable values may themselves use `${...}`, e.g.
//! `[var] SKYRIM_DIR = "${game:SkyrimSE}"`.
//!
//! [`detect_skyrim_dir`] is only used without manifest, to import the game
//! scripts of a workspace that has no configuration at all.

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    sync::OnceLock,
};

/// The variable naming the Skyrim installation directory.
pub const SKYRIM_DIR: &str = "SKYRIM_DIR";

/// Nested `${...}` expansions deeper than this are considered a cycle.
const MAX_DEPTH: usize = 16;

/// Where the value of a variable comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VarSource {
    /// Editor settings or server arguments.
    Editor,
    /// `[var]` of `papyrus.local.toml`.
    Local,
    /// `[var]` of `papyrus.toml`.
    Manifest,
    /// The process environment (`${env:NAME}`).
    Environment,
    /// A workspace folder (`${workspaceFolder}`).
    Workspace,
    /// A game installation from the registry (`${game:SkyrimSE}`).
    Registry,
}

impl fmt::Display for VarSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            VarSource::Editor => "editor settings",
            VarSource::Local => "papyrus.local.toml",
            VarSource::Manifest => "papyrus.toml",
            VarSource::Environment => "environment",
            VarSource::Workspace => "workspace folder",
            VarSource::Registry => "registry",
        })
    }
}

/// The variable sources of one machine.
#[derive(Clone, Debug)]
pub struct Variables {
    /// Values from the editor settings or the server arguments.
    pub editor: BTreeMap<String, String>,
    /// Values of `[var]` of `papyrus.local.toml`.
    pub local: BTreeMap<String, String>,
    /// Values of `[var]` of `papyrus.toml`.
    pub manifest: BTreeMap<String, String>,
    /// Reads the process environment, for `${env:NAME}` and `~`.
    pub process_env: fn(&str) -> Option<String>,
    /// The workspace folders, for `${workspaceFolder}`.
    pub workspace_folders: Vec<PathBuf>,
}

impl Default for Variables {
    fn default() -> Self {
        Variables::new(BTreeMap::new())
    }
}

fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

impl Variables {
    /// The variables of this machine, with `editor` values on top.
    pub fn new(editor: BTreeMap<String, String>) -> Variables {
        Variables {
            editor,
            local: BTreeMap::new(),
            manifest: BTreeMap::new(),
            process_env,
            workspace_folders: Vec::new(),
        }
    }

    /// Sets the workspace folders.
    pub fn with_workspace_folders(mut self, folders: Vec<PathBuf>) -> Variables {
        self.workspace_folders = folders;
        self
    }

    /// The raw (unexpanded) value of a variable, or of `env:NAME`.
    fn raw(&self, name: &str) -> Option<(String, VarSource)> {
        if let Some(env) = name.strip_prefix("env:") {
            return (self.process_env)(env).map(|v| (v, VarSource::Environment));
        }
        if let Some(game) = name.strip_prefix("game:") {
            let dir = game_dir(game)?;
            return Some((dir.display().to_string(), VarSource::Registry));
        }
        if name == "workspaceFolder" {
            let dir = self.workspace_folders.first()?;
            return Some((dir.display().to_string(), VarSource::Workspace));
        }
        if let Some(folder) = name.strip_prefix("workspaceFolder:") {
            let dir = self.workspace_folders.iter().find(|d| {
                d.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(folder))
            })?;
            return Some((dir.display().to_string(), VarSource::Workspace));
        }
        if let Some(v) = self.editor.get(name).filter(|v| !v.is_empty()) {
            return Some((v.clone(), VarSource::Editor));
        }
        if let Some(v) = self.local.get(name) {
            return Some((v.clone(), VarSource::Local));
        }
        if let Some(v) = self.manifest.get(name) {
            return Some((v.clone(), VarSource::Manifest));
        }
        None
    }

    /// Looks a variable (or `env:NAME`) up and expands its value.
    pub fn lookup(&self, name: &str) -> Option<(String, VarSource)> {
        let (raw, source) = self.raw(name)?;
        let value =
            self.expand_inner(&raw, &mut BTreeMap::new(), &mut vec![name.to_owned()]).ok()?;
        Some((value, source))
    }

    /// Expands `${NAME}` and `${env:NAME}` and a leading `~`, recording the
    /// variables used into `used`.
    pub fn expand(
        &self,
        value: &str,
        used: &mut BTreeMap<String, (String, VarSource)>,
    ) -> Result<String, String> {
        self.expand_inner(value, used, &mut Vec::new())
    }

    fn expand_inner(
        &self,
        value: &str,
        used: &mut BTreeMap<String, (String, VarSource)>,
        stack: &mut Vec<String>,
    ) -> Result<String, String> {
        let mut out = String::with_capacity(value.len());
        let mut rest = value;
        if let Some(after) = rest.strip_prefix('~')
            && (after.is_empty() || after.starts_with(['/', '\\']))
        {
            let home = (self.process_env)("HOME")
                .or_else(|| (self.process_env)("USERPROFILE"))
                .ok_or("cannot expand `~`: no home directory")?;
            out.push_str(&home);
            rest = after;
        }
        while let Some(start) = rest.find("${") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let end = after.find('}').ok_or("unterminated `${` in value")?;
            let name = after[..end].trim();
            if stack.iter().any(|s| s == name) || stack.len() >= MAX_DEPTH {
                return Err(format!("variable `{name}` refers to itself"));
            }
            let (raw, source) = self.raw(name).ok_or_else(|| undefined(name))?;
            stack.push(name.to_owned());
            let value = self.expand_inner(&raw, used, stack)?;
            stack.pop();
            out.push_str(&value);
            used.insert(name.to_owned(), (value, source));
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// The installation directory of a game, by [`skyrim_data_dir::Runtime`] name
/// (ignoring case).
pub fn game_dir(name: &str) -> Option<PathBuf> {
    let runtime = game_runtime(name)?;
    let data = skyrim_data_dir::get_skyrim_data_dir(runtime).ok()?;
    data.parent().map(Path::to_path_buf)
}

fn game_runtime(name: &str) -> Option<skyrim_data_dir::Runtime> {
    skyrim_data_dir::Runtime::ALL
        .into_iter()
        .find(|rt| rt.as_str().eq_ignore_ascii_case(name.trim()))
}

fn undefined(name: &str) -> String {
    if let Some(game) = name.strip_prefix("game:") {
        return match game_runtime(game) {
            Some(_) if !cfg!(windows) => {
                format!("`${{game:{game}}}` reads the Windows registry and only works on Windows")
            }
            Some(rt) => format!("{rt} is not installed: no registry entry found"),
            None => {
                let names: Vec<&str> =
                    skyrim_data_dir::Runtime::ALL.iter().map(|r| r.as_str()).collect();
                format!("unknown game `{game}`, expected one of {}", names.join(", "))
            }
        };
    }
    if name.starts_with("workspaceFolder") {
        return format!("no workspace folder for `${{{name}}}`");
    }
    match name.strip_prefix("env:") {
        Some(env) => format!("environment variable `{env}` is not set"),
        None => format!(
            "undefined variable `{name}`: set it in the `papyrus.var` editor setting \
             (or `papyrus_analyzer --var {name}=...`), or in `[var]` of papyrus.local.toml \
             or papyrus.toml"
        ),
    }
}
/// Finds the Skyrim installation directory, once: the `Installed Path` of the
/// Bethesda registry key (Special Edition, then VR, then Legendary Edition),
/// else the default Steam library.
pub fn detect_skyrim_dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        use skyrim_data_dir::{Runtime, get_skyrim_data_dir};
        let from_registry = [Runtime::Se, Runtime::Vr, Runtime::Le].into_iter().find_map(|rt| {
            // The registry gives `<game>/Data`; variables name the game directory.
            let data = get_skyrim_data_dir(rt).ok()?;
            data.parent().map(Path::to_path_buf).filter(|dir| dir.is_dir())
        });
        from_registry.or_else(|| {
            [
                "C:/Program Files (x86)/Steam/steamapps/common/Skyrim Special Edition",
                "C:/Program Files/Steam/steamapps/common/Skyrim Special Edition",
            ]
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_dir())
        })
    })
    .clone()
}
/// The vanilla script sources below a Skyrim installation that exist:
/// `Data/Scripts/Source` (SKSE, older layout) and `Data/Source/Scripts`.
pub fn game_script_dirs(skyrim_dir: &Path) -> Vec<PathBuf> {
    [["Data", "Source", "Scripts"], ["Data", "Scripts", "Source"]]
        .iter()
        .map(|parts| parts.iter().fold(skyrim_dir.to_path_buf(), |dir, p| dir.join(p)))
        .filter(|d| d.is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_env(name: &str) -> Option<String> {
        match name {
            "SKYRIM_DIR" => Some("/env/skyrim".to_owned()),
            "OTHER" => Some("/env/other".to_owned()),
            _ => None,
        }
    }

    fn vars() -> Variables {
        Variables {
            editor: BTreeMap::new(),
            local: BTreeMap::new(),
            manifest: BTreeMap::new(),
            process_env: fake_env,
            workspace_folders: vec![PathBuf::from("/ws/first"), PathBuf::from("/ws/second")],
        }
    }

    #[test]
    fn priority() {
        let mut v = vars();
        // Undefined variables are not guessed.
        assert_eq!(v.lookup("SKYRIM_DIR"), None);
        v.manifest.insert("SKYRIM_DIR".into(), "manifest".into());
        assert_eq!(v.lookup("SKYRIM_DIR"), Some(("manifest".into(), VarSource::Manifest)));
        v.local.insert("SKYRIM_DIR".into(), "local".into());
        assert_eq!(v.lookup("SKYRIM_DIR"), Some(("local".into(), VarSource::Local)));
        v.editor.insert("SKYRIM_DIR".into(), "editor".into());
        assert_eq!(v.lookup("SKYRIM_DIR"), Some(("editor".into(), VarSource::Editor)));
        // Plain names never read the environment.
        assert_eq!(v.lookup("OTHER"), None);
        assert_eq!(v.lookup("env:OTHER"), Some(("/env/other".into(), VarSource::Environment)));
        assert_eq!(v.lookup("workspaceFolder").unwrap().0, "/ws/first");
        assert_eq!(v.lookup("workspaceFolder:second").unwrap().1, VarSource::Workspace);
        assert_eq!(v.lookup("workspaceFolder:nope"), None);
    }

    #[test]
    fn game_prefix() {
        let mut used = BTreeMap::new();
        let err = vars().expand("${game:Oblivion}", &mut used).unwrap_err();
        assert!(err.contains("expected one of SkyrimLE, SkyrimSE"), "{err}");
        // Installed or not depends on the machine; both outcomes are valid.
        match vars().expand("${game:skyrimse}", &mut used) {
            Ok(dir) => assert_eq!(used["game:skyrimse"].1, VarSource::Registry, "{dir}"),
            Err(err) => assert!(
                err.contains("SkyrimSE is not installed") || err.contains("only works on Windows"),
                "{err}"
            ),
        }
    }

    #[test]
    fn expand_nested_and_env() {
        let mut v = vars();
        v.manifest.insert("SKYRIM_DIR".into(), "${env:SKYRIM_DIR}".into());
        v.manifest.insert("SCRIPTS".into(), "${SKYRIM_DIR}/Data/Scripts".into());
        let mut used = BTreeMap::new();
        assert_eq!(
            v.expand("${SCRIPTS}/Source", &mut used).unwrap(),
            "/env/skyrim/Data/Scripts/Source"
        );
        assert_eq!(used["SCRIPTS"].1, VarSource::Manifest);
        assert_eq!(used["env:SKYRIM_DIR"].1, VarSource::Environment);

        assert!(v.expand("${NOPE}", &mut used).unwrap_err().contains("papyrus.var"));
        assert!(v.expand("${env:NOPE}", &mut used).unwrap_err().contains("not set"));
        v.manifest.insert("A".into(), "${B}".into());
        v.manifest.insert("B".into(), "${A}".into());
        assert!(v.expand("${A}", &mut used).unwrap_err().contains("refers to itself"));
    }
}
