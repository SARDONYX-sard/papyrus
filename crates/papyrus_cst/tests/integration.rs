#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use papyrus_cst::{display_error, parse_papyrus};
    use rayon::prelude::*;

    #[derive(Debug)]
    struct TestResult {
        path: PathBuf,
        status: Status,
        message: Option<String>,
    }

    #[derive(Debug)]
    enum Status {
        Ok,
        ParseError,
        Panic,
    }

    fn strip_utf8_bom(mut s: String) -> String {
        const BOM: char = '\u{feff}';
        if s.starts_with(BOM) {
            let len = BOM.len_utf8();
            s.drain(..len);
        }
        s
    }

    fn run_file(path: &Path) -> TestResult {
        let res = std::panic::catch_unwind(|| {
            let bytes = fs::read(path).unwrap();
            let src = strip_utf8_bom(auto_charset::decode_to_utf8(bytes).unwrap());

            let (_tree, errors) = parse_papyrus(&src);

            if !errors.is_empty() {
                let path_s = display_error::to_filename(path);
                let msg = display_error::display_errors(&src, &path_s, &errors);
                return TestResult {
                    path: path.to_path_buf(),
                    status: Status::ParseError,
                    message: Some(msg),
                };
            }

            TestResult {
                path: path.to_path_buf(),
                status: Status::Ok,
                message: None,
            }
        });

        match res {
            Ok(v) => v,
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };

                TestResult {
                    path: path.to_path_buf(),
                    status: Status::Panic,
                    message: Some(msg),
                }
            }
        }
    }

    fn collect_psc_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();

            if path.is_dir() {
                collect_psc_files(&path, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some("psc") {
                out.push(path);
            }
        }
    }

    fn format_results(results: &[TestResult]) -> String {
        let mut out = String::new();

        for r in results {
            if matches!(r.status, Status::Ok) {
                continue;
            }
            out.push_str("========================================\n");
            out.push_str(&format!("file: {}\n", r.path.display()));
            out.push_str(&format!("status: {:?}\n", r.status));

            if let Some(msg) = &r.message {
                out.push_str("message:\n");
                out.push_str(msg);
                out.push('\n');
            }
        }

        out
    }

    #[test]
    fn full_fixture_recursive() {
        let mut files = Vec::new();
        collect_psc_files(Path::new("../../tests"), &mut files);

        assert!(!files.is_empty(), "no .psc files found");

        let results: Vec<TestResult> = files
            .par_iter()
            .map(|path: &PathBuf| run_file(path))
            .collect();

        let report = format_results(&results);

        fs::create_dir_all("../../target/tests").unwrap();
        fs::write("../../target/integration_results.log", &report).unwrap();

        let failed = results
            .iter()
            .filter(|r| !matches!(r.status, Status::Ok))
            .count();
        if failed > 0 {
            panic!("error count: {failed}\n{report}");
        }
        // assert_eq!(failed, 0, "some fixtures failed");
    }
}
