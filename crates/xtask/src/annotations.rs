pub fn run(_args: impl Iterator<Item = String>) -> std::process::ExitCode {
    let mut failures = Vec::new();
    for hit in scan("DIVERGED:", DIVERGED_CATEGORIES) {
        failures.push(hit);
    }
    for hit in scan("RUST_ONLY:", RUST_ONLY_CATEGORIES) {
        failures.push(hit);
    }
    for hit in scan_malformed("DIVERGED (") {
        failures.push(hit);
    }
    if failures.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        for f in &failures {
            eprintln!("{}", f);
        }
        std::process::ExitCode::from(1)
    }
}

const DIVERGED_CATEGORIES: &[&str] = &[
    "language-forced",
    "dependency-forced",
    "upstream-gap-forced",
    "performance-forced",
];

const RUST_ONLY_CATEGORIES: &[&str] = &[
    "language-forced-utility",
    "performance-forced-alternative",
    "dependency-forced",
];

fn scan_malformed(tag: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let walker = walkdir::WalkDir::new("crates")
        .into_iter()
        .filter_entry(|e| !e.path().starts_with("crates/xtask"))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"));
    for entry in walker {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if line.contains(tag) {
                failures.push(format!(
                    "{}:{} malformed annotation '{}' — use 'DIVERGED: (category)' form",
                    entry.path().display(),
                    i + 1,
                    tag.trim(),
                ));
            }
        }
    }
    failures
}

fn scan(tag: &str, valid_categories: &[&str]) -> Vec<String> {
    let mut failures = Vec::new();
    let walker = walkdir::WalkDir::new("crates")
        .into_iter()
        .filter_entry(|e| !e.path().starts_with("crates/xtask"))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"));
    for entry in walker {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let lines: Vec<_> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.contains(tag) {
                let window = lines[i..(i + 4).min(lines.len())].join("\n");
                if !valid_categories
                    .iter()
                    .any(|c| window.contains(&format!("({c})")))
                {
                    failures.push(format!(
                        "{}:{} {} missing category tag",
                        entry.path().display(),
                        i + 1,
                        tag
                    ));
                }
            }
        }
    }
    failures
}
