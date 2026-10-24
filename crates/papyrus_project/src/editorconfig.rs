//! The `.editorconfig` properties that matter to the formatter.
//!
//! Files are read from the directory of the script up to the first file with
//! `root = true`. Closer files win, and inside a file later sections win, as
//! the [EditorConfig specification](https://spec.editorconfig.org/) says.
//! Supported globs: `*`, `**`, `?`, `[abc]`, `[!abc]`, `{a,b}` and `{1..9}`.

use std::path::Path;

/// Formatting properties of `.editorconfig`; `None` when unset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorConfig {
    /// `indent_style = tab | space`: `Some(true)` for tabs.
    pub use_tabs: Option<bool>,
    /// `indent_size` (or `tab_width` when `indent_size = tab`).
    pub indent_size: Option<u8>,
    /// `end_of_line = lf | crlf`: `Some(true)` for `crlf`.
    pub crlf: Option<bool>,
    /// `insert_final_newline`.
    pub final_newline: Option<bool>,
}

impl EditorConfig {
    /// Reads the properties that apply to `file`.
    pub fn for_file(file: &Path) -> EditorConfig {
        let mut files = Vec::new();
        for dir in file.ancestors().skip(1) {
            let Ok(text) = std::fs::read_to_string(dir.join(".editorconfig")) else { continue };
            let root = is_root(&text);
            files.push((dir.to_path_buf(), text));
            if root {
                break;
            }
        }
        // Farthest first, so that closer files override.
        let mut props: Vec<(String, String)> = Vec::new();
        for (dir, text) in files.iter().rev() {
            let Ok(relative) = file.strip_prefix(dir) else { continue };
            let relative = relative.to_string_lossy().replace('\\', "/");
            props.extend(properties(text, &relative));
        }
        let mut out = EditorConfig::default();
        let mut tab_width = None;
        let mut size_is_tab = false;
        for (key, value) in props {
            match key.as_str() {
                "indent_style" => out.use_tabs = Some(value == "tab"),
                "indent_size" if value == "tab" => {
                    out.indent_size = None;
                    size_is_tab = true;
                }
                "indent_size" => {
                    out.indent_size = value.parse().ok();
                    size_is_tab = false;
                }
                "tab_width" => tab_width = value.parse().ok(),
                "end_of_line" => out.crlf = Some(value == "crlf"),
                "insert_final_newline" => out.final_newline = Some(value == "true"),
                _ => {}
            }
        }
        if size_is_tab || (out.indent_size.is_none() && out.use_tabs == Some(true)) {
            out.indent_size = out.indent_size.or(tab_width);
        }
        out
    }

    /// Applies the set properties to formatter options.
    pub fn apply(&self, options: &mut papyrus_fmt::FormatOptions) {
        match (self.use_tabs, self.indent_size) {
            (Some(true), _) => options.indent = papyrus_fmt::Indent::Tabs,
            (Some(false), size) => {
                options.indent =
                    papyrus_fmt::Indent::Spaces(size.unwrap_or(match options.indent {
                        papyrus_fmt::Indent::Spaces(n) => n,
                        papyrus_fmt::Indent::Tabs => 4,
                    }));
            }
            (None, Some(size)) => {
                if let papyrus_fmt::Indent::Spaces(_) = options.indent {
                    options.indent = papyrus_fmt::Indent::Spaces(size);
                }
            }
            (None, None) => {}
        }
        if let Some(crlf) = self.crlf {
            options.newline =
                if crlf { papyrus_fmt::Newline::CrLf } else { papyrus_fmt::Newline::Lf };
        }
        if let Some(final_newline) = self.final_newline {
            options.final_newline = final_newline;
        }
    }
}

fn is_root(text: &str) -> bool {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            return false;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case("root")
        {
            return v.trim().eq_ignore_ascii_case("true");
        }
    }
    false
}

/// The properties of the sections of `text` matching `relative`, in order.
/// Keys and values are lowercased, as the specification requires.
fn properties(text: &str, relative: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut matching = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            matching = section_matches(section, relative);
            continue;
        }
        if matching && let Some((k, v)) = line.split_once('=') {
            out.push((k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase()));
        }
    }
    out
}

/// A glob without `/` matches the file name in any directory.
fn section_matches(glob: &str, relative: &str) -> bool {
    let glob = glob.strip_prefix('/').unwrap_or(glob);
    if glob.contains('/') {
        glob_match(glob.as_bytes(), relative.as_bytes())
    } else {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        glob_match(glob.as_bytes(), name.as_bytes())
    }
}

/// Matches an EditorConfig glob, ignoring ASCII case like Windows paths.
fn glob_match(glob: &[u8], text: &[u8]) -> bool {
    match glob.first() {
        None => text.is_empty(),
        Some(b'*') if glob.get(1) == Some(&b'*') => {
            let rest = &glob[2..];
            (0..=text.len()).any(|i| glob_match(rest, &text[i..]))
        }
        Some(b'*') => {
            let rest = &glob[1..];
            (0..=text.len())
                .take_while(|&i| i == 0 || text[i - 1] != b'/')
                .any(|i| glob_match(rest, &text[i..]))
        }
        Some(b'?') => !text.is_empty() && text[0] != b'/' && glob_match(&glob[1..], &text[1..]),
        Some(b'[') => {
            let Some(end) = glob.iter().position(|&b| b == b']') else {
                return literal(glob, text);
            };
            let (set, negate) = match glob.get(1) {
                Some(b'!') => (&glob[2..end], true),
                _ => (&glob[1..end], false),
            };
            let Some(&c) = text.first() else { return false };
            let mut hit = false;
            let mut i = 0;
            while i < set.len() {
                if i + 2 < set.len() && set[i + 1] == b'-' {
                    hit |= (set[i]..=set[i + 2]).contains(&c.to_ascii_lowercase())
                        || (set[i]..=set[i + 2]).contains(&c);
                    i += 3;
                } else {
                    hit |= set[i].eq_ignore_ascii_case(&c);
                    i += 1;
                }
            }
            hit != negate && glob_match(&glob[end + 1..], &text[1..])
        }
        Some(b'{') => {
            let Some(end) = glob.iter().position(|&b| b == b'}') else {
                return literal(glob, text);
            };
            let inner = std::str::from_utf8(&glob[1..end]).unwrap_or("");
            let rest = &glob[end + 1..];
            if let Some((lo, hi)) = inner.split_once("..")
                && let (Ok(lo), Ok(hi)) = (lo.parse::<i64>(), hi.parse::<i64>())
            {
                return (lo..=hi).any(|n| {
                    let mut alt = n.to_string().into_bytes();
                    alt.extend_from_slice(rest);
                    glob_match(&alt, text)
                });
            }
            inner.split(',').any(|alt| {
                let mut alt = alt.as_bytes().to_vec();
                alt.extend_from_slice(rest);
                glob_match(&alt, text)
            })
        }
        Some(_) => literal(glob, text),
    }
}

fn literal(glob: &[u8], text: &[u8]) -> bool {
    !text.is_empty() && glob[0].eq_ignore_ascii_case(&text[0]) && glob_match(&glob[1..], &text[1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(section_matches("*", "a/b/Foo.psc"));
        assert!(section_matches("*.psc", "a/b/Foo.psc"));
        assert!(section_matches("*.{psc,flg}", "Foo.flg"));
        assert!(!section_matches("*.{psc,flg}", "Foo.txt"));
        assert!(section_matches("scripts/**.psc", "scripts/a/b/Foo.psc"));
        assert!(!section_matches("scripts/*.psc", "scripts/a/Foo.psc"));
        assert!(section_matches("Foo[0-9].psc", "Foo3.psc"));
        assert!(section_matches("Foo[!0-9].psc", "FooX.psc"));
        assert!(section_matches("v{1..3}.psc", "v2.psc"));
        assert!(section_matches("FOO.PSC", "foo.psc"));
    }

    #[test]
    fn reads_nested_files() {
        let dir = std::env::temp_dir().join(format!("papyrus-editorconfig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(
            dir.join(".editorconfig"),
            "root = true\n[*]\nindent_style = space\nindent_size = 2\nend_of_line = lf\n[*.psc]\ninsert_final_newline = true\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("sub/.editorconfig"),
            "[*.psc]\nindent_style = tab\nindent_size = tab\ntab_width = 8\n",
        )
        .unwrap();
        let cfg = EditorConfig::for_file(&dir.join("sub/Foo.psc"));
        assert_eq!(
            cfg,
            EditorConfig {
                use_tabs: Some(true),
                indent_size: Some(8),
                crlf: Some(false),
                final_newline: Some(true)
            }
        );
        let top = EditorConfig::for_file(&dir.join("Bar.psc"));
        assert_eq!((top.use_tabs, top.indent_size), (Some(false), Some(2)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
