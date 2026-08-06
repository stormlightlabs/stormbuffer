use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_directory() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after the Unix epoch")
        .as_nanos();
    let path = env::temp_dir().join(format!("stormbuffer-doc-examples-{suffix}"));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

fn binary(name: &str) -> PathBuf {
    if let Some(path) = env::var_os(format!("CARGO_BIN_EXE_{name}")) {
        return PathBuf::from(path);
    }

    let test_binary = env::current_exe().expect("locate the process test binary");
    test_binary
        .parent()
        .and_then(Path::parent)
        .expect("locate Cargo's debug directory")
        .join(name)
}

fn run(name: &str, directory: &Path, arguments: &[&str]) -> Output {
    let home = directory.join("home");
    let data = directory.join("data");
    let cache = directory.join("cache");
    Command::new(binary(name))
        .current_dir(directory)
        .args(arguments)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &data)
        .env("APPDATA", &data)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("STORMBUFFER_TEST_MODE", "1")
        .output()
        .expect("run documented CLI example")
}

fn markdown_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read documentation directory") {
        let path = entry.expect("read documentation entry").path();
        if path.is_dir() {
            markdown_files(&path, files);
        } else if path.extension() == Some(OsStr::new("md")) {
            files.push(path);
        }
    }
}

fn documented_cli_examples() -> Vec<(String, Vec<String>)> {
    let docs_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/content/docs");
    let mut files = Vec::new();
    markdown_files(&docs_directory, &mut files);
    files.sort();

    let mut examples = Vec::new();
    for file in files {
        let contents = fs::read_to_string(&file).expect("read documentation page");
        let mut in_shell_block = false;
        for line in contents.lines() {
            match line.trim() {
                "```sh" => in_shell_block = true,
                "```" if in_shell_block => in_shell_block = false,
                _ if in_shell_block => {
                    let parts: Vec<_> = line.split_whitespace().map(String::from).collect();
                    if matches!(
                        parts.first().map(String::as_str),
                        Some("stormbuffer" | "stormbuf" | "sbuf")
                    ) {
                        examples.push((file.display().to_string(), parts));
                    }
                }
                _ => {}
            }
        }
    }
    examples
}

#[test]
fn documented_cli_examples_stay_executable() {
    let examples = documented_cli_examples();
    assert!(
        examples.len() >= 8,
        "expected the documented CLI smoke examples to be present"
    );

    let root = temporary_directory();
    let mut current_source = String::new();
    let mut page_number = 0;
    let mut page_root = root.clone();
    for (source, parts) in examples {
        if source != current_source {
            current_source.clone_from(&source);
            page_number += 1;
            page_root = root.join(format!("page-{page_number}"));
            fs::create_dir_all(&page_root).expect("create documentation page test directory");
        }
        let name = &parts[0];
        let arguments: Vec<_> = parts[1..].iter().map(String::as_str).collect();
        let output = run(name, &page_root, &arguments);
        assert!(
            output.status.success(),
            "documented command from {source} failed: {name} {}\n{}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(root).expect("remove test directory");
}
