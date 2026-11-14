use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "risc-c-cli-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}
#[test]
fn stdout_csv_and_failure_exit() {
    let version = Command::new(env!("CARGO_BIN_EXE_spice"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        format!("spice {}", env!("CARGO_PKG_VERSION"))
    );
    let d = dir();
    let input = d.join("divider.cir");
    std::fs::write(
        &input,
        "Title\nV1 a 0 10\nR1 a out 1k\nR2 out 0 1k\n.op\n.end",
    )
    .unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_spice"))
        .arg("run")
        .arg(&input)
        .args(["--output", "-"])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = String::from_utf8(run.stdout).unwrap();
    assert!(text.contains("v(out)"));
    assert_eq!(text.lines().count(), 2);
    let fail = Command::new(env!("CARGO_BIN_EXE_spice"))
        .arg("run")
        .arg(&input)
        .args(["--analysis", "ac"])
        .output()
        .unwrap();
    assert!(!fail.status.success());
    std::fs::remove_dir_all(d).unwrap();
}
#[test]
fn simulation_failure_does_not_publish_results() {
    let d = dir();
    let input = d.join("failure.cir");
    let output = d.join("results.csv");
    std::fs::write(&input, "Title\nV1 a 0 1\nR1 a 0 1k\n.op\n.tran 0 1\n.end").unwrap();
    let run = Command::new(env!("CARGO_BIN_EXE_spice"))
        .arg("run")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(!run.status.success());
    assert!(!output.exists());
    assert!(!d.join("results.op.csv").exists());
    std::fs::remove_dir_all(d).unwrap();
}
