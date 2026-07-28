use std::io::Write;
use std::process::{Command, Output};

use datadiff_test_support::*;

// Reuse the library-less approach: run the compiled binary and also exercise
// the library modules through the binary's behavior.

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_datadiff"))
}

fn write_temp(dir: &std::path::Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    path
}

fn run(args: &[&std::path::Path], extra: &[&str]) -> Output {
    let mut cmd = bin();
    for a in args {
        cmd.arg(a);
    }
    for e in extra {
        cmd.arg(e);
    }
    cmd.arg("--no-color");
    cmd.output().unwrap()
}

#[test]
fn reordered_keys_is_no_change() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.json",
        r#"{"name": "app", "replicas": 3, "tags": ["x", "y"]}"#,
    );
    let b = write_temp(
        &dir,
        "b.json",
        r#"{"tags": ["x", "y"], "replicas": 3, "name": "app"}"#,
    );
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("0 changes"), "stdout: {stdout}");
}

#[test]
fn reordered_array_with_key_is_no_change() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.json",
        r#"{"users": [{"id": 1, "name": "ann"}, {"id": 2, "name": "bob"}]}"#,
    );
    let b = write_temp(
        &dir,
        "b.json",
        r#"{"users": [{"id": 2, "name": "bob"}, {"id": 1, "name": "ann"}]}"#,
    );
    let out = run(&[&a, &b], &["--key", "id"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("0 changes"), "stdout: {stdout}");
}

#[test]
fn env_key_variable_matches_key_flag() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.json",
        r#"{"users": [{"id": 1, "name": "ann"}, {"id": 2, "name": "bob"}]}"#,
    );
    let b = write_temp(
        &dir,
        "b.json",
        r#"{"users": [{"id": 2, "name": "bob"}, {"id": 1, "name": "ann"}]}"#,
    );
    let out = bin()
        .arg(&a)
        .arg(&b)
        .arg("--no-color")
        .env("DATADIFF_KEY", "id")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("0 changes"), "stdout: {stdout}");
}

#[test]
fn nested_change_reports_data_path() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"services": {"web": {"replicas": 2}}}"#);
    let b = write_temp(&dir, "b.json", r#"{"services": {"web": {"replicas": 5}}}"#);
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("~ services.web.replicas: 2 → 5"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("1 changes (0 added, 0 removed, 1 modified)"));
}

#[test]
fn int_and_float_are_equal() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"version": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"version": 1.0}"#);
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn csv_with_key_modified_added_removed_rows() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.csv",
        "id,name,price\n100,apple,10\n200,pear,20\n300,plum,30\n",
    );
    let b = write_temp(
        &dir,
        "b.csv",
        "id,name,price\n200,pear,20\n100,apple,12\n400,cherry,40\n",
    );
    let out = run(&[&a, &b], &["--key", "id"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("~ $[id=100].price: 10 → 12"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("+ $[id=400]:"), "stdout: {stdout}");
    assert!(stdout.contains("- $[id=300]:"), "stdout: {stdout}");
    assert!(
        stdout.contains("3 changes (1 added, 1 removed, 1 modified)"),
        "stdout: {stdout}"
    );
}

#[test]
fn csv_without_key_matches_by_row_number() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.csv", "id,price\n1,10\n2,20\n");
    let b = write_temp(&dir, "b.csv", "id,price\n2,20\n1,10\n");
    // Same rows reordered: without --key this is a positional diff.
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn missing_file_exits_2() {
    let dir = tempfile_dir();
    let b = write_temp(&dir, "b.json", "{}");
    let missing = dir.join("nope.json");
    let out = run(&[&missing, &b], &[]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn invalid_format_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", "{not json");
    let b = write_temp(&dir, "b.json", "{}");
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn added_and_removed_object_keys() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "keep: 1\ndrop: 2\n");
    let b = write_temp(&dir, "b.yaml", "keep: 1\nnew: 3\n");
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("- drop: 2"), "stdout: {stdout}");
    assert!(stdout.contains("+ new: 3"), "stdout: {stdout}");
}

#[test]
fn show_unchanged_prints_equal_lines() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"same": true, "diff": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"same": true, "diff": 2}"#);
    let out = run(&[&a, &b], &["--show-unchanged"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("= same"), "stdout: {stdout}");
    assert!(stdout.contains("~ diff: 1 → 2"), "stdout: {stdout}");
}

mod datadiff_test_support {
    pub fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "datadiff-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
