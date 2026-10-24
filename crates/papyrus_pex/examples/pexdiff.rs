//! Compares two `.pex` files after [`normalize`](papyrus_pex::normalize):
//!
//! ```text
//! cargo run -p papyrus_pex --example pexdiff -- [--loose] A.pex B.pex
//! ```
//!
//! Prints the differing lines (`-` only in A, `+` only in B) and exits with
//! 1 when the files differ. `--loose` ignores which temporaries are used.

#![allow(clippy::print_stdout, clippy::print_stderr)]

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let loose = args.iter().any(|a| a == "--loose");
    args.retain(|a| a != "--loose");
    let [a, b] = args.as_slice() else {
        eprintln!("usage: pexdiff [--loose] A.pex B.pex");
        std::process::exit(2);
    };
    let load = |path: &str| {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let pex =
            papyrus_pex::PexFile::from_bytes(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        papyrus_pex::normalize::normalize_with(&pex, loose)
    };
    let (a, b) = (load(a), load(b));
    let a: Vec<&str> = a.lines().collect();
    let b: Vec<&str> = b.lines().collect();
    let diff = diff(&a, &b);
    if diff.is_empty() {
        println!("same");
        return;
    }
    for line in diff {
        println!("{line}");
    }
    std::process::exit(1);
}

/// A line diff by longest common subsequence, with one line of context.
fn diff(a: &[&str], b: &[&str]) -> Vec<String> {
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] =
                if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            ops.push(('+', b[j]));
            j += 1;
        } else {
            ops.push(('-', a[i]));
            i += 1;
        }
    }
    let changed: Vec<bool> = ops.iter().map(|(c, _)| *c != ' ').collect();
    let mut out = Vec::new();
    for (k, (c, line)) in ops.iter().enumerate() {
        let near = changed[k.saturating_sub(1)..(k + 2).min(ops.len())].iter().any(|&x| x);
        if near {
            out.push(format!("{c} {line}"));
        }
    }
    if changed.iter().all(|&c| !c) { Vec::new() } else { out }
}
