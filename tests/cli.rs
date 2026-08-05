//! Drives the built binary as a child process.
//!
//! This covers what only a real process shows: the options read from the
//! environment, and the psql run that follows the printed command line.

use std::path::PathBuf;
use std::process::{Command, Output};

/// A DSN no server answers, so that psql fails without needing one.
const UNREACHABLE_DSN: &str = "host=127.0.0.1 port=1 dbname=nowhere";

/// Creates a directory holding a single `stats.sql` file using one variable.
fn queries_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sqlrunner-cli-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stats.sql"), "SELECT :'day';").unwrap();
    dir
}

/// Runs the binary with `arguments`, and only the given environment variables
/// besides the `PATH` psql is looked up in.
fn sqlrunner(arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sqlrunner"));
    command.args(arguments).env_clear();
    command.env("PATH", std::env::var("PATH").unwrap_or_default());

    for (name, value) in environment {
        command.env(name, value);
    }

    command.output().unwrap()
}

/// Returns the standard streams of a run as text.
fn streams(output: &Output) -> (&str, &str) {
    (
        std::str::from_utf8(&output.stdout).unwrap(),
        std::str::from_utf8(&output.stderr).unwrap(),
    )
}

#[test]
fn sql_dir_comes_from_the_environment() {
    let dir = queries_dir("sql-dir");

    let output = sqlrunner(&[], &[("SQLRUNNER_SQL_DIR", dir.to_str().unwrap())]);
    let (stdout, _) = streams(&output);

    assert!(output.status.success());
    assert_eq!(stdout, "FILE       VARIABLES\nstats.sql  day\n");
}

#[test]
fn a_missing_sql_dir_is_still_required() {
    let output = sqlrunner(&[], &[]);
    let (_, stderr) = streams(&output);

    assert!(!output.status.success());
    assert!(stderr.contains("--sql-dir"));
}

#[test]
fn running_prints_the_command_line_then_runs_it() {
    let dir = queries_dir("runs-psql");

    let output = sqlrunner(
        &["stats.sql", "day=2026-08-05"],
        &[
            ("SQLRUNNER_SQL_DIR", dir.to_str().unwrap()),
            ("SQLRUNNER_DSN", UNREACHABLE_DSN),
        ],
    );
    let (stdout, stderr) = streams(&output);

    // One line per option, then a blank line before whatever psql writes.
    // The escapes coloring the command line are left out, stdout being a pipe.
    assert_eq!(
        stdout,
        format!(
            "psql \\\n    \
                 -d '{UNREACHABLE_DSN}' \\\n    \
                 -v day=2026-08-05 \\\n    \
                 -f {}\n\n",
            dir.join("stats.sql").display()
        )
    );

    // psql really ran, and reported on the inherited stderr that it could not
    // reach the server; its exit status is the one sqlrunner exits with.
    assert!(stderr.contains("connection to server"), "stderr: {stderr}");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn the_command_line_wins_over_the_environment() {
    let dir = queries_dir("precedence");

    let output = sqlrunner(
        &[
            "--sql-dir",
            dir.to_str().unwrap(),
            "--dsn",
            UNREACHABLE_DSN,
            "stats.sql",
            "day=2026-08-05",
        ],
        &[
            ("SQLRUNNER_SQL_DIR", "/does/not/exist"),
            ("SQLRUNNER_DSN", "host=other dbname=from_env"),
        ],
    );
    let (stdout, _) = streams(&output);

    assert!(
        stdout.contains(&format!("-d '{UNREACHABLE_DSN}'")),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("from_env"), "stdout: {stdout}");
}

#[test]
fn an_invalid_invocation_runs_nothing() {
    let dir = queries_dir("invalid-invocation");

    // The `day` variable of the file is left unset.
    let output = sqlrunner(
        &["stats.sql"],
        &[("SQLRUNNER_SQL_DIR", dir.to_str().unwrap())],
    );
    let (stdout, stderr) = streams(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.is_empty(), "stdout: {stdout}");
    assert_eq!(stderr, "sqlrunner: stats.sql: unset variables: day\n");
}
