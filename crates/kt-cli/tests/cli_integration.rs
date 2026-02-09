//! CLI integration tests
//!
//! Tests the k-terminus CLI using assert_cmd.

use assert_cmd::Command;
use predicates::prelude::*;

fn k_terminus() -> Command {
    Command::cargo_bin("k-terminus")
        .expect("Failed to locate k-terminus binary - ensure it's built before running tests")
}

#[test]
fn test_cli_help() {
    k_terminus()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("k-terminus"))
        .stdout(predicate::str::contains(
            "Distributed terminal session manager",
        ));
}

#[test]
fn test_cli_version() {
    k_terminus()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("k-terminus"));
}

#[test]
fn test_cli_serve_help() {
    k_terminus()
        .args(["serve", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("orchestrator"));
}

#[test]
fn test_cli_join_help() {
    k_terminus()
        .args(["join", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("agent"));
}

#[test]
fn test_cli_list_help() {
    k_terminus()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("machines"));
}

#[test]
fn test_cli_connect_help() {
    k_terminus()
        .args(["connect", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("session"));
}

#[test]
fn test_cli_status_help() {
    k_terminus().args(["status", "--help"]).assert().success();
}

#[test]
fn test_cli_config_help() {
    k_terminus()
        .args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("config"));
}

#[test]
fn test_cli_config_show() {
    // This should work even without an orchestrator running
    k_terminus().args(["config", "show"]).assert().success();
}

#[test]
fn test_cli_unknown_command() {
    k_terminus()
        .arg("nonexistent-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn test_cli_list() {
    // The CLI auto-starts the orchestrator if needed, so this should succeed.
    // Uses default port 22230 - run with --test-threads=1 to avoid conflicts.
    k_terminus()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("machines").or(predicate::str::contains("Machines")));
}

#[test]
fn test_cli_status() {
    // The CLI auto-starts the orchestrator if needed, so this should succeed.
    // Uses default port 22230 - run with --test-threads=1 to avoid conflicts.
    k_terminus()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Orchestrator").or(predicate::str::contains("running")));
}

#[test]
fn test_cli_connect_missing_machine() {
    // Connect requires a machine argument
    k_terminus().arg("connect").assert().failure();
}

#[test]
fn test_cli_join_missing_host() {
    // Join requires a host argument
    k_terminus().arg("join").assert().failure();
}
