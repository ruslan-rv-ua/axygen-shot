use assert_cmd::Command;
use tempfile::TempDir;

fn shot_cmd() -> Command {
    Command::cargo_bin("shot").unwrap()
}

#[test]
fn walk_up_finds_config_in_parent() {
    let parent = TempDir::new().unwrap();
    let child = parent.path().join("subdir");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(
        parent.path().join("shot.toml"),
        "process = \"nonexistent-for-test.exe\"\n",
    )
    .unwrap();
    // --check should find the config and try to resolve the window (will fail at resolve)
    let output = shot_cmd()
        .arg("--check")
        .current_dir(&child)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail at window resolution, not at config loading
    assert!(
        stderr.contains("window-not-found"),
        "Expected window-not-found error (walk-up should find config), got: {}",
        stderr
    );
}

#[test]
fn walk_up_stops_at_git_boundary() {
    let root = TempDir::new().unwrap();
    let child = root.path().join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(root.path().join("shot.toml"), "process = \"test.exe\"\n").unwrap();
    // Create .git boundary in child — should prevent walk-up
    std::fs::create_dir(child.join(".git")).unwrap();
    let output = shot_cmd()
        .arg("--check")
        .current_dir(&child)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should fail with "No target" because config was not found (blocked by .git)
    assert!(
        stderr.contains("No target") || stderr.contains("config-error"),
        "Expected 'No target' or 'config-error', got: {}",
        stderr
    );
}
