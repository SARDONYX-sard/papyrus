// SPDX-License-Identifier: Apache-2.0 OR MIT
// ref: https://github.com/apollographql/apollo-rs 8b64f55db8843e6f90e087e6bc77a91b8c45a537
//! See <https://github.com/matklad/cargo-xtask/>.
//!
//! This binary defines various auxiliary build commands, which are not
//! expressible with just `cargo`. Notably, it provides tests via `cargo test -p xtask`
//! for code generation and `cargo xtask install` for installation of
//! rust-analyzer server and client.
//!
//! This binary is integrated into the `cargo` command line by using an alias in
//! `.cargo/config`.

#![warn(rust_2018_idioms, unused_lifetimes)]
#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::disallowed_methods,
    clippy::disallowed_types
)]

use anyhow::{Result, bail};
use std::path::Path;
use xshell::{Shell, cmd};

mod codegen;
mod cst_src;
mod utils;

fn main() -> anyhow::Result<()> {
    let flags = Xtask::from_env_or_exit();

    match flags.subcommand {
        XtaskCmd::Codegen(cmd) => cmd.run(),
    }
}

xflags::xflags! {
    cmd xtask {
        cmd codegen {
            optional --check
        }
    }
}

impl Codegen {
    pub fn run(self) -> anyhow::Result<()> {
        crate::codegen::generate()
    }
}

fn root_path() -> std::path::PathBuf {
    std::path::Path::new(
        &std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned()),
    )
    .ancestors()
    .nth(1)
    .unwrap()
    .to_path_buf()
}

fn rustfmt() -> Result<()> {
    let sh = Shell::new()?;

    let out = cmd!(sh, "rustfmt --version").read()?;
    if !out.contains("stable") {
        bail!(
            "Failed to run rustfmt from toolchain 'stable'. \
             Please run `rustup component add rustfmt --toolchain stable` to install it.",
        )
    }
    Ok(())
}

fn reformat(text: &str) -> Result<String> {
    let sh = Shell::new()?;
    let _e = sh.push_env("RUSTUP_TOOLCHAIN", "stable");
    rustfmt()?;
    let stdout = cmd!(sh, "rustfmt --config fn_single_line=true").stdin(text).read()?;
    Ok(format!(
        "{}\n\n{}\n",
        "//! This is a generated file, please do not edit manually. Changes can be
//! made in code generation that lives in `xtask` top-level dir.",
        stdout
    ))
}

pub(crate) fn ensure_file_contents(file: &Path, contents: &str) -> Result<()> {
    match std::fs::read_to_string(file) {
        Ok(old_contents) if normalize_newlines(&old_contents) == normalize_newlines(contents) => {
            return Ok(());
        }
        _ => (),
    }
    let display_path = file.strip_prefix(root_path()).unwrap_or(file);
    eprintln!(
        "\n\x1b[31;1merror\x1b[0m: {} was not up-to-date, updating\n",
        display_path.display()
    );
    if std::env::var("CI").is_ok() {
        eprintln!("    NOTE: run `cargo test` locally and commit the updated files\n");
    }
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(file, contents).unwrap();
    // bail!(
    //     "{} was not up to date and has been updated. Make sure to re-run cargo check and cargo test to accomodate the updates.",
    //     file.display()
    // );
    Ok(())
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}
