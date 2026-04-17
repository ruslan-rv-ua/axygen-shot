use assert_cmd::Command;
use tempfile::TempDir;

fn shot_cmd() -> Command {
    Command::cargo_bin("shot").unwrap()
}

#[test]
fn init_creates_shot_toml() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .arg("--init")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("status: ok"));
    let toml_path = dir.path().join("shot.toml");
    assert!(toml_path.exists());
    let content = std::fs::read_to_string(&toml_path).unwrap();
    assert!(
        content.contains('#'),
        "shot.toml should contain inline comments"
    );
}

#[test]
fn init_with_process_fills_field() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .args(["--init", "--process=myapp.exe"])
        .current_dir(dir.path())
        .assert()
        .success();
    let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
    assert!(content.lines().any(|l| l == r#"process = "myapp.exe""#));
}

#[test]
fn init_with_title_fills_field() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .args(["--init", "--title=MyApp"])
        .current_dir(dir.path())
        .assert()
        .success();
    let content = std::fs::read_to_string(dir.path().join("shot.toml")).unwrap();
    assert!(content.lines().any(|l| l == r#"title   = "MyApp""#));
}

#[test]
fn init_fails_if_shot_toml_exists() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("shot.toml"), "existing").unwrap();
    shot_cmd()
        .arg("--init")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn init_creates_gitignore_entry() {
    let dir = TempDir::new().unwrap();
    shot_cmd()
        .arg("--init")
        .current_dir(dir.path())
        .assert()
        .success();
    let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(content.contains("screenshots/"));
}
