use std::path::Path;
use std::process::Command;

fn mem_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mem"))
}

fn setup_temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().unwrap()
}

fn init_mems(dir: &Path) {
    let status = mem_cmd()
        .current_dir(dir)
        .arg("init")
        .status()
        .expect("failed to run mem init");
    assert!(status.success(), "mem init failed");
}

#[test]
fn test_init_creates_directory() {
    let temp = setup_temp_dir();

    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("init")
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    assert!(temp.path().join(".mems").exists());
    assert!(temp.path().join(".mems/archive").exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Initialized"));
}

#[test]
fn test_init_fails_if_exists() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("init")
        .output()
        .expect("failed to run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn test_add_and_show() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add a mem
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["add", "test/doc", "-c", "Hello world", "-t", "Test Title"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created: test/doc"));

    // Show the mem
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "test/doc"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Test Title"));
    assert!(stdout.contains("Hello world"));
}

#[test]
fn test_add_with_stdin() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add via stdin
    let mut child = mem_cmd()
        .current_dir(temp.path())
        .args(["add", "stdin-test"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"Content from stdin")
        .unwrap();

    let output = child.wait_with_output().expect("failed to wait");
    assert!(output.status.success());

    // Verify content
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "stdin-test"])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Content from stdin"));
}

#[test]
fn test_add_duplicate_fails() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add first time
    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "dup", "-c", "First"])
        .status()
        .unwrap();

    // Add second time without force should fail
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["add", "dup", "-c", "Second"])
        .output()
        .expect("failed to run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn test_add_with_force_overwrites() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add first time
    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "force-test", "-c", "First"])
        .status()
        .unwrap();

    // Add with force
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["add", "force-test", "-c", "Second", "--force"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());

    // Verify new content
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "force-test"])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Second"));
    assert!(!stdout.contains("First"));
}

#[test]
fn test_edit() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add
    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "edit-test", "-c", "Original", "-t", "Original Title"])
        .status()
        .unwrap();

    // Edit content
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["edit", "edit-test", "-c", "Updated content"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());

    // Verify
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "edit-test"])
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Updated content"));
    assert!(stdout.contains("Original Title")); // Title unchanged
}

#[test]
fn test_rm() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add
    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "to-delete", "-c", "Delete me"])
        .status()
        .unwrap();

    // Delete
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["rm", "to-delete"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());

    // Verify gone
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "to-delete"])
        .output()
        .expect("failed to run");

    assert!(!output.status.success());
}

#[test]
fn test_ls() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    // Add some mems
    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "a/first", "-c", "Content", "--tags", "tag1"])
        .status()
        .unwrap();

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "b/second", "-c", "Content"])
        .status()
        .unwrap();

    // List all
    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("ls")
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a/first"));
    assert!(stdout.contains("b/second"));
    assert!(stdout.contains("[tag1]"));
}

#[test]
fn test_ls_path_filter() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "docs/one", "-c", "Content"])
        .status()
        .unwrap();

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "notes/two", "-c", "Content"])
        .status()
        .unwrap();

    // List only docs
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["ls", "docs"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("docs/one"));
    assert!(!stdout.contains("notes/two"));
}

#[test]
fn test_find() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "rust-notes", "-c", "Rust programming language notes"])
        .status()
        .unwrap();

    mem_cmd()
        .current_dir(temp.path())
        .args([
            "add",
            "python-notes",
            "-c",
            "Python programming language notes",
        ])
        .status()
        .unwrap();

    // Find rust
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["find", "rust"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rust-notes"));
    assert!(!stdout.contains("python-notes"));
}

#[test]
fn test_tree() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "arch/decisions/adr-001", "-c", "Decision 1"])
        .status()
        .unwrap();

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "arch/decisions/adr-002", "-c", "Decision 2"])
        .status()
        .unwrap();

    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("tree")
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("arch/"));
    assert!(stdout.contains("decisions/"));
    assert!(stdout.contains("adr-001"));
    assert!(stdout.contains("adr-002"));
}

#[test]
fn test_archive() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "to-archive", "-c", "Archive me"])
        .status()
        .unwrap();

    // Archive
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["archive", "to-archive"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());

    // Should not appear in ls
    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("ls")
        .output()
        .expect("failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("to-archive"));

    // But file should exist in archive
    assert!(temp.path().join(".mems/archive/to-archive.md").exists());
}

#[test]
fn test_lint_passes() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "valid", "-c", "Valid content"])
        .status()
        .unwrap();

    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("lint")
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No issues found"));
}

#[test]
fn test_lint_broken_link() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "with-link", "-c", "See [other](nonexistent.md)"])
        .status()
        .unwrap();

    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("lint")
        .output()
        .expect("failed to run");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("broken link"));
}

#[test]
fn test_json_output() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    mem_cmd()
        .current_dir(temp.path())
        .args(["add", "json-test", "-c", "Content", "--tags", "a,b"])
        .status()
        .unwrap();

    // Test show --json
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "json-test", "--json"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert_eq!(json["path"], "json-test");
    assert_eq!(json["content"], "Content");
    assert!(json["tags"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("a")));

    // Test ls --json
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["ls", "--json"])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("invalid JSON");
    assert!(json.as_array().unwrap().len() == 1);
}

#[test]
fn test_missing_mems_directory() {
    let temp = setup_temp_dir();
    // Don't init - should fail

    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("ls")
        .output()
        .expect("failed to run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no .mems/"));
}

#[test]
fn test_show_nonexistent() {
    let temp = setup_temp_dir();
    init_mems(temp.path());

    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "nonexistent"])
        .output()
        .expect("failed to run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"));
}

#[test]
fn test_multi_dir_ls() {
    let temp_a = setup_temp_dir();
    let temp_b = setup_temp_dir();
    init_mems(temp_a.path());
    init_mems(temp_b.path());

    mem_cmd()
        .current_dir(temp_a.path())
        .args(["add", "from-a", "-c", "Content A"])
        .status()
        .unwrap();

    mem_cmd()
        .current_dir(temp_b.path())
        .args(["add", "from-b", "-c", "Content B"])
        .status()
        .unwrap();

    let dir_a = temp_a.path().join(".mems");
    let dir_b = temp_b.path().join(".mems");

    let output = mem_cmd()
        .args([
            "ls",
            "--dir",
            dir_a.to_str().unwrap(),
            "--dir",
            dir_b.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("from-a"));
    assert!(stdout.contains("from-b"));
    // Should have directory prefixes in multi-dir mode
    assert!(stdout.contains("["));
}

#[test]
fn test_workflow_init_add_edit_archive() {
    let temp = setup_temp_dir();

    // Init
    assert!(mem_cmd()
        .current_dir(temp.path())
        .arg("init")
        .status()
        .unwrap()
        .success());

    // Add
    assert!(mem_cmd()
        .current_dir(temp.path())
        .args(["add", "workflow", "-c", "Initial", "-t", "Workflow Test"])
        .status()
        .unwrap()
        .success());

    // Edit
    assert!(mem_cmd()
        .current_dir(temp.path())
        .args(["edit", "workflow", "-c", "Updated"])
        .status()
        .unwrap()
        .success());

    // Verify edit
    let output = mem_cmd()
        .current_dir(temp.path())
        .args(["show", "workflow"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("Updated"));

    // Archive
    assert!(mem_cmd()
        .current_dir(temp.path())
        .args(["archive", "workflow"])
        .status()
        .unwrap()
        .success());

    // Verify archived (not in ls)
    let output = mem_cmd()
        .current_dir(temp.path())
        .arg("ls")
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stdout).contains("workflow"));
}
