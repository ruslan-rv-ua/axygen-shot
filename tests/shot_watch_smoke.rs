use assert_cmd::Command;

/// Verifies shot-watch.exe builds and clap parses --help.
/// We cannot assert on stdout because the binary uses windows-subsystem=windows
/// and stdout may not appear in pipe under some configurations.
#[test]
fn shot_watch_help_exits_zero() {
    Command::cargo_bin("shot-watch")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn shot_watch_version_exits_zero() {
    Command::cargo_bin("shot-watch")
        .unwrap()
        .arg("--version")
        .assert()
        .success();
}
