//! Process-level contracts for the released application's output and write failures.

use std::process::Command;

#[test]
fn the_application_emits_one_greeting_line_and_succeeds() {
    let output = Command::new(env!("CARGO_BIN_EXE_thysalion"))
        .output()
        .expect("launch the application binary");
    assert!(output.status.success(), "application failed: {output:?}");
    assert_eq!(
        output.stdout, b"Hello from Thysalion!\n",
        "the application greeting is exactly one newline-terminated line"
    );
    assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
}

/// Linux's full device deterministically rejects a write without a pipe race.
#[cfg(target_os = "linux")]
#[test]
fn a_stdout_write_failure_exits_unsuccessfully() {
    let devices = cap_std::fs::Dir::open_ambient_dir("/dev", cap_std::ambient_authority())
        .expect("open the device directory");
    let full = devices
        .open_with("full", cap_std::fs::OpenOptions::new().write(true))
        .expect("open the full device for the child stdout");
    let output = Command::new(env!("CARGO_BIN_EXE_thysalion"))
        .stdout(full.into_std())
        .output()
        .expect("launch the application with failing stdout");
    assert!(
        !output.status.success(),
        "stdout failure was lost: {output:?}"
    );
    assert!(!output.stderr.is_empty(), "write failure must be reported");
}
