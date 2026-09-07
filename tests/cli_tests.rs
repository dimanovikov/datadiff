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

fn run_with_stdin(args: &[&std::path::Path], extra: &[&str], stdin: &str) -> Output {
    let mut cmd = bin();
    for a in args {
        cmd.arg(a);
    }
    for e in extra {
        cmd.arg(e);
    }
    cmd.arg("--no-color");
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    {
        let mut sin = child.stdin.take().unwrap();
        // A usage error exits 2 without reading stdin, closing the pipe while we
        // are still writing to it. A broken pipe here is the expected outcome,
        // not a test failure.
        match sin.write_all(stdin.as_bytes()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
            Err(e) => panic!("writing to child stdin failed: {e}"),
        }
        // Dropping `sin` closes stdin so the child sees EOF and can finish reading.
    }
    child.wait_with_output().unwrap()
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
fn empty_old_file_reports_everything_added() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", "");
    let b = write_temp(
        &dir,
        "b.json",
        r#"{"replicas": 5, "spec": {"image": "app:1"}}"#,
    );
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("+ replicas: 5"), "stdout: {stdout}");
    assert!(stdout.contains("+ spec:"), "stdout: {stdout}");
    assert!(
        stdout.contains("2 changes (2 added, 0 removed, 0 modified)"),
        "stdout: {stdout}"
    );
}

#[test]
fn empty_new_file_reports_everything_removed() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"replicas": 5}"#);
    let b = write_temp(&dir, "b.json", "");
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("- replicas: 5"), "stdout: {stdout}");
}

#[test]
fn dev_null_side_is_treated_as_missing() {
    // git external diff passes /dev/null for added/deleted files
    let dir = tempfile_dir();
    let b = write_temp(&dir, "b.json", r#"{"replicas": 5}"#);
    let dev_null = std::path::Path::new("/dev/null").to_path_buf();
    let out = run(&[&dev_null, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("+ replicas: 5"), "stdout: {stdout}");
}

#[test]
fn stdin_side_diffs_against_file() {
    let dir = tempfile_dir();
    let b = write_temp(&dir, "b.json", r#"{"replicas": 5, "name": "app"}"#);
    let dash = std::path::Path::new("-").to_path_buf();
    let out = run_with_stdin(&[&dash, &b], &["--format", "json"], r#"{"replicas": 3}"#);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("~ replicas: 3 → 5"), "stdout: {stdout}");
    assert!(stdout.contains("+ name: \"app\""), "stdout: {stdout}");
}

#[test]
fn stdin_no_differences_exits_0() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "replicas: 3\nname: app\n");
    let dash = std::path::Path::new("-").to_path_buf();
    // Key order differs from the file; semantically the same document.
    let out = run_with_stdin(
        &[&a, &dash],
        &["--format", "yaml"],
        "name: app\nreplicas: 3\n",
    );
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("0 changes"), "stdout: {stdout}");
}

#[test]
fn stdin_without_format_exits_2() {
    let dir = tempfile_dir();
    let b = write_temp(&dir, "b.json", "{}");
    let dash = std::path::Path::new("-").to_path_buf();
    let out = run_with_stdin(&[&dash, &b], &[], "{}");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("cannot detect format of stdin"),
        "stderr: {stderr}"
    );
}

#[test]
fn both_sides_stdin_exits_2() {
    let dash = std::path::Path::new("-").to_path_buf();
    let out = run_with_stdin(&[&dash, &dash], &["--format", "json"], "{}");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("stdin can only be read once"),
        "stderr: {stderr}"
    );
}

#[test]
fn empty_stdin_side_reports_everything_added() {
    let dir = tempfile_dir();
    let b = write_temp(&dir, "b.json", r#"{"replicas": 5}"#);
    let dash = std::path::Path::new("-").to_path_buf();
    let out = run_with_stdin(&[&dash, &b], &["--format", "json"], "");
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("+ replicas: 5"), "stdout: {stdout}");
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

#[test]
fn json_output_reports_changes() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "keep: 1\ndrop: 2\nchange: 3\n");
    let b = write_temp(&dir, "b.yaml", "keep: 1\nnew: 4\nchange: 5\n");
    let out = run(&[&a, &b], &["--output", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    assert_eq!(doc["summary"]["added"], 1);
    assert_eq!(doc["summary"]["removed"], 1);
    assert_eq!(doc["summary"]["modified"], 1);
    assert_eq!(doc["summary"]["total"], 3);
    let changes = doc["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 3, "stdout: {stdout}");
    let modified = changes
        .iter()
        .find(|c| c["type"] == "modified")
        .expect("a modified entry");
    assert_eq!(modified["path"], "change");
    assert_eq!(modified["old"], 3);
    assert_eq!(modified["new"], 5);
    let added = changes.iter().find(|c| c["type"] == "added").unwrap();
    assert_eq!(added["path"], "new");
    assert!(added["old"].is_null());
    assert_eq!(added["new"], 4);
}

#[test]
fn json_output_empty_diff_exits_0() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"same": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"same": 1}"#);
    let out = run(&[&a, &b], &["--output", "json"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["changes"].as_array().unwrap().len(), 0);
    assert_eq!(doc["summary"]["total"], 0);
}

#[test]
fn json_output_show_unchanged_includes_unchanged_entries() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"same": true, "diff": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"same": true, "diff": 2}"#);
    let out = run(&[&a, &b], &["--output", "json", "--show-unchanged"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changes = doc["changes"].as_array().unwrap();
    assert!(
        changes
            .iter()
            .any(|c| c["type"] == "unchanged" && c["path"] == "same"),
        "stdout: {stdout}"
    );
}

#[test]
fn patch_round_trip_json() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.json",
        r#"{"keep": 1, "drop": 2, "change": 3, "nested": {"x": "old"}}"#,
    );
    let b = write_temp(
        &dir,
        "b.json",
        r#"{"keep": 1, "new": 4, "change": 5, "nested": {"x": "new"}}"#,
    );
    // Generate the patch.
    let diff_out = run(&[&a, &b], &["--output", "json"]);
    assert_eq!(diff_out.status.code(), Some(1));
    let patch = write_temp(
        &dir,
        "patch.json",
        &String::from_utf8(diff_out.stdout).unwrap(),
    );
    // Apply it to `a`.
    let out = bin().arg("patch").arg(&a).arg(&patch).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let patched = write_temp(
        &dir,
        "patched.json",
        &String::from_utf8(out.stdout).unwrap(),
    );
    // The patched document must have no differences against `b`.
    let check = run(&[&patched, &b], &[]);
    assert_eq!(check.status.code(), Some(0));
}

#[test]
fn patch_round_trip_yaml_with_key() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.yaml",
        "users:\n  - id: 1\n    name: ann\n  - id: 2\n    name: bob\n  - id: 3\n    name: cid\n",
    );
    let b = write_temp(
        &dir,
        "b.yaml",
        "users:\n  - id: 2\n    name: bob\n  - id: 1\n    name: anne\n  - id: 4\n    name: dan\n",
    );
    let diff_out = run(&[&a, &b], &["--key", "id", "--output", "json"]);
    assert_eq!(diff_out.status.code(), Some(1));
    let patch = write_temp(
        &dir,
        "patch.json",
        &String::from_utf8(diff_out.stdout).unwrap(),
    );
    let out = bin().arg("patch").arg(&a).arg(&patch).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let patched = write_temp(
        &dir,
        "patched.json",
        &String::from_utf8(out.stdout).unwrap(),
    );
    let check = run(&[&patched, &b], &["--key", "id", "--format", "yaml"]);
    assert_eq!(check.status.code(), Some(0));
}

#[test]
fn patch_csv_with_key() {
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
    let diff_out = run(&[&a, &b], &["--key", "id", "--output", "json"]);
    let patch = write_temp(
        &dir,
        "patch.json",
        &String::from_utf8(diff_out.stdout).unwrap(),
    );
    let out = bin().arg("patch").arg(&a).arg(&patch).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let rows = doc.as_array().unwrap();
    assert_eq!(rows.len(), 3, "stdout: {stdout}");
    let apple = rows.iter().find(|r| r["id"] == 100).unwrap();
    assert_eq!(apple["price"], 12);
    assert!(rows.iter().any(|r| r["id"] == 400));
    assert!(!rows.iter().any(|r| r["id"] == 300));
}

#[test]
fn patch_unresolvable_path_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"keep": 1}"#);
    let patch = write_temp(
        &dir,
        "patch.json",
        r#"{"changes": [{"type": "removed", "path": "nope.key", "old": 1, "new": null}], "summary": {"added": 0, "removed": 1, "modified": 0, "total": 1}}"#,
    );
    let out = bin().arg("patch").arg(&a).arg(&patch).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn patch_invalid_json_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"keep": 1}"#);
    let patch = write_temp(&dir, "patch.json", "{not json");
    let out = bin().arg("patch").arg(&a).arg(&patch).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn exit_zero_overrides_differences_exit_code() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"v": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"v": 2}"#);
    let out = run(&[&a, &b], &["--exit-zero"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("1 changes"), "stdout: {stdout}");
    // Errors still exit 2.
    let missing = dir.join("nope.json");
    let out = run(&[&missing, &b], &["--exit-zero"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn xml_change_reports_data_path() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.xml",
        r#"<server><port>8080</port><tls enabled="false" version="1.2"/></server>"#,
    );
    let b = write_temp(
        &dir,
        "b.xml",
        r#"<server><port>9090</port><tls version="1.3" enabled="false"/></server>"#,
    );
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    // The root element name is dropped by quick-xml; element text is
    // reported under `$text`, attributes under `@name`.
    assert!(stdout.contains("~ port.$text:"), "stdout: {stdout}");
    assert!(stdout.contains("~ tls.@version:"), "stdout: {stdout}");
}

#[test]
fn xml_reordered_keys_and_attributes_is_no_change() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.xml",
        r#"<server><host>a</host><port>1</port></server>"#,
    );
    let b = write_temp(
        &dir,
        "b.xml",
        r#"<server><port>1</port><host>a</host></server>"#,
    );
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn invalid_xml_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.xml", "<unclosed>");
    let b = write_temp(&dir, "b.xml", "<a/>");
    let out = run(&[&a, &b], &[]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn convert_yaml_to_json() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n  name: app\n");
    let out_path = dir.join("out.json");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out_path).unwrap()).unwrap();
    assert_eq!(doc["spec"]["replicas"], 3);
    assert_eq!(doc["spec"]["name"], "app");
}

#[test]
fn convert_json_to_csv_round_trip() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.json",
        r#"[{"id": 1, "price": 10}, {"id": 2, "price": 20}]"#,
    );
    let out_path = dir.join("out.csv");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    // Converting back to JSON yields the same values.
    let back = dir.join("back.json");
    let out = bin()
        .arg("convert")
        .arg(&out_path)
        .arg(&back)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let check = run(&[&a, &back], &[]);
    assert_eq!(check.status.code(), Some(0));
}

#[test]
fn convert_non_object_root_to_toml_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"[1, 2, 3]"#);
    let out_path = dir.join("out.toml");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn convert_nested_object_to_csv_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"[{"id": 1, "nested": {"x": 1}}]"#);
    let out_path = dir.join("out.csv");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn fail_on_matching_pattern_exits_1() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n  note: a\n");
    let b = write_temp(&dir, "b.yaml", "spec:\n  replicas: 5\n  note: b\n");
    let out = run(&[&a, &b], &["--fail-on", "spec.replicas"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn fail_on_non_matching_pattern_exits_0_but_prints() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n  note: a\n");
    let b = write_temp(&dir, "b.yaml", "spec:\n  replicas: 5\n  note: b\n");
    let out = run(&[&a, &b], &["--fail-on", "spec.image"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("2 changes"), "stdout: {stdout}");
}

#[test]
fn fail_on_glob_pattern() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n");
    let b = write_temp(&dir, "b.yaml", "spec:\n  replicas: 5\n");
    let out = run(&[&a, &b], &["--fail-on", "*.replicas"]);
    assert_eq!(out.status.code(), Some(1));
    // A prefix pattern matches nested paths.
    let out = run(&[&a, &b], &["--fail-on", "spec"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn fail_on_env_variable() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n");
    let b = write_temp(&dir, "b.yaml", "spec:\n  replicas: 5\n");
    let out = bin()
        .arg(&a)
        .arg(&b)
        .arg("--no-color")
        .env("DATADIFF_FAIL_ON", "spec.replicas")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn patch_output_replaces_modified_value() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n");
    let b = write_temp(&dir, "b.yaml", "spec:\n  replicas: 5\n");
    let out = run(&[&a, &b], &["--output", "patch"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    let ops = doc.as_array().unwrap();
    assert_eq!(
        ops,
        &vec![serde_json::json!({"op": "replace", "path": "/spec/replicas", "value": 5})],
        "stdout: {stdout}"
    );
}

#[test]
fn patch_output_empty_diff_exits_0() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"same": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"same": 1}"#);
    let out = run(&[&a, &b], &["--output", "patch"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc.as_array().unwrap().len(), 0, "stdout: {stdout}");
}

#[test]
fn patch_output_skips_unchanged_with_show_unchanged() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"same": true, "diff": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"same": true, "diff": 2}"#);
    let out = run(&[&a, &b], &["--output", "patch", "--show-unchanged"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let ops = doc.as_array().unwrap();
    assert_eq!(ops.len(), 1, "stdout: {stdout}");
    assert_eq!(ops[0]["path"], "/diff");
}

#[test]
fn patch_output_with_key_resolves_indices() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.json",
        r#"{"users": [{"id": 1, "n": "a"}, {"id": 2, "n": "b"}, {"id": 3, "n": "c"}, {"id": 4, "n": "d"}]}"#,
    );
    let b = write_temp(
        &dir,
        "b.json",
        r#"{"users": [{"id": 1, "n": "a2"}, {"id": 3, "n": "c"}, {"id": 5, "n": "e"}]}"#,
    );
    let out = run(&[&a, &b], &["--key", "id", "--output", "patch"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}\n{stdout}"));
    let ops = doc.as_array().unwrap();
    assert_eq!(
        ops,
        &vec![
            // Key lookups resolve to old-tree indices; replaces first.
            serde_json::json!({"op": "replace", "path": "/users/0/n", "value": "a2"}),
            // Removals of one array go in descending index order, so
            // applying them does not shift the remaining indices.
            serde_json::json!({"op": "remove", "path": "/users/3"}),
            serde_json::json!({"op": "remove", "path": "/users/1"}),
            // A keyed element absent in the old tree appends via `-`.
            serde_json::json!({"op": "add", "path": "/users/-", "value": {"id": 5, "n": "e"}}),
        ],
        "stdout: {stdout}"
    );
}

#[test]
fn convert_yaml_to_xml() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.yaml", "spec:\n  replicas: 3\n  name: app\n");
    let out_path = dir.join("out.xml");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let xml = std::fs::read_to_string(&out_path).unwrap();
    // The root element name is not retained by parsing; it is always `root`.
    assert!(xml.contains("<root>"), "xml: {xml}");
    assert!(xml.contains("<replicas>3</replicas>"), "xml: {xml}");
}

#[test]
fn convert_xml_round_trip() {
    let dir = tempfile_dir();
    let a = write_temp(
        &dir,
        "a.xml",
        r#"<server><port>8080</port><tls enabled="false" version="1.2"/></server>"#,
    );
    let out_path = dir.join("out.xml");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "stderr: {:?}", out.stderr);
    let xml = std::fs::read_to_string(&out_path).unwrap();
    // Attributes survive the round trip via the `@name` mapping.
    assert!(xml.contains(r#"enabled="false""#), "xml: {xml}");
    // The written file diffs clean against the original.
    let check = run(&[&a, &out_path], &[]);
    assert_eq!(
        check.status.code(),
        Some(0),
        "stdout: {:?}",
        String::from_utf8(check.stdout)
    );
}

#[test]
fn convert_non_object_root_to_xml_exits_2() {
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"[1, 2, 3]"#);
    let out_path = dir.join("out.xml");
    let out = bin()
        .arg("convert")
        .arg(&a)
        .arg(&out_path)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("the document root is not an object"),
        "stderr: {stderr}"
    );
}

// ---- git integration ----

/// Run the binary with raw string arguments (the git subcommands take their
/// own argument shapes rather than the OLD/NEW pair).
fn run_raw(args: &[&str]) -> Output {
    let mut cmd = bin();
    for a in args {
        cmd.arg(a);
    }
    cmd.output().unwrap()
}

/// git invokes a diff driver as: path old-file old-hex old-mode new-file
/// new-hex new-mode. Only the second and fifth carry the two versions.
fn git_seven(path: &str, old: &std::path::Path, new: &std::path::Path) -> Vec<String> {
    vec![
        path.to_string(),
        old.display().to_string(),
        "0f74d10".to_string(),
        "100644".to_string(),
        new.display().to_string(),
        "593c038".to_string(),
        "100644".to_string(),
    ]
}

#[test]
fn git_diff_compares_the_second_and_fifth_arguments() {
    let dir = tempfile_dir();
    let old = write_temp(&dir, "old.json", r#"{"replicas": 3}"#);
    let new = write_temp(&dir, "new.json", r#"{"replicas": 5}"#);

    let seven = git_seven("deploy.json", &old, &new);
    let mut args = vec!["git-diff", "--no-color"];
    args.extend(seven.iter().map(String::as_str));
    let out = run_raw(&args);

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("~ replicas: 3 \u{2192} 5"),
        "stdout: {stdout}"
    );
}

#[test]
fn git_diff_exits_zero_when_changes_are_found() {
    // git treats a non-zero exit from a diff driver as a failure of the whole
    // command, so the driver must report differences through its output only.
    let dir = tempfile_dir();
    let old = write_temp(&dir, "old.json", r#"{"replicas": 3}"#);
    let new = write_temp(&dir, "new.json", r#"{"replicas": 5}"#);

    let seven = git_seven("deploy.json", &old, &new);
    let mut args = vec!["git-diff", "--no-color"];
    args.extend(seven.iter().map(String::as_str));
    let out = run_raw(&args);

    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn git_diff_takes_the_format_from_the_named_path() {
    // git stages the two versions in temporary files. Their names are not
    // guaranteed to carry the original extension, so the format has to come
    // from the first argument, which is always the real path.
    let dir = tempfile_dir();
    let old = write_temp(&dir, "blob_a", "replicas: 3\n");
    let new = write_temp(&dir, "blob_b", "replicas: 5\n");

    let seven = git_seven("deploy.yaml", &old, &new);
    let mut args = vec!["git-diff", "--no-color"];
    args.extend(seven.iter().map(String::as_str));
    let out = run_raw(&args);

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "stdout: {stdout}");
    assert!(
        stdout.contains("~ replicas: 3 \u{2192} 5"),
        "stdout: {stdout}"
    );
}

#[test]
fn git_diff_prints_the_path_as_a_header() {
    // git prints nothing of its own around an external driver's output, so a
    // diff touching several files would otherwise be unattributable.
    let dir = tempfile_dir();
    let old = write_temp(&dir, "old.json", r#"{"replicas": 3}"#);
    let new = write_temp(&dir, "new.json", r#"{"replicas": 5}"#);

    let seven = git_seven("deploy/app.json", &old, &new);
    let mut args = vec!["git-diff", "--no-color"];
    args.extend(seven.iter().map(String::as_str));
    let out = run_raw(&args);

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("deploy/app.json"), "stdout: {stdout}");
}

#[test]
fn git_diff_reports_an_added_file_entry_by_entry() {
    let dir = tempfile_dir();
    let new = write_temp(&dir, "new.json", r#"{"replicas": 5}"#);
    let dev_null = std::path::Path::new("/dev/null").to_path_buf();

    let seven = git_seven("deploy.json", &dev_null, &new);
    let mut args = vec!["git-diff", "--no-color"];
    args.extend(seven.iter().map(String::as_str));
    let out = run_raw(&args);

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("+ replicas: 5"), "stdout: {stdout}");
}

#[test]
fn normalize_ignores_key_order() {
    // The point of textconv: two files that differ only in key order have to
    // normalize to byte-identical text, or git still shows a diff.
    let dir = tempfile_dir();
    let a = write_temp(&dir, "a.json", r#"{"b": 2, "a": 1}"#);
    let b = write_temp(&dir, "b.json", r#"{"a": 1, "b": 2}"#);

    let first = run_raw(&["normalize", &a.display().to_string()]);
    let second = run_raw(&["normalize", &b.display().to_string()]);

    assert_eq!(first.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        String::from_utf8(second.stdout).unwrap()
    );
}

#[test]
fn normalize_keeps_the_source_format() {
    // A YAML file has to normalize to YAML: git line-diffs the two outputs,
    // and rendering them as JSON would make every YAML diff unreadable.
    let dir = tempfile_dir();
    let file = write_temp(&dir, "deploy.yaml", "replicas: 3\nimage: app\n");

    let out = run_raw(&["normalize", &file.display().to_string()]);

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("replicas: 3"), "stdout: {stdout}");
    assert!(!stdout.contains('{'), "expected YAML, got: {stdout}");
}

#[test]
fn normalize_of_an_empty_file_prints_nothing() {
    let dir = tempfile_dir();
    let file = write_temp(&dir, "empty.json", "");

    let out = run_raw(&["normalize", &file.display().to_string()]);

    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8(out.stdout).unwrap().trim().is_empty());
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
