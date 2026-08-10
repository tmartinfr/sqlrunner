//! Drives the built binary as a child process.
//!
//! This covers what only a real process shows: the options read from the
//! environment, and the psql run that follows the printed command line.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

/// A DSN no server answers, so that psql fails without needing one.
const UNREACHABLE_DSN: &str = "host=127.0.0.1 port=1 dbname=nowhere";

/// Creates a directory holding a single `stats.sql` file which uses one variable
/// and carries a description on its first line.
fn queries_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sqlrunner-cli-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stats.sql"), "-- Daily counters\nSELECT :'day';").unwrap();
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

/// Runs the binary as `sqlrunner` does, `input` being fed to its standard input.
fn sqlrunner_with_input(
    arguments: &[&str],
    environment: &[(&str, &str)],
    input: &str,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sqlrunner"));
    command.args(arguments).env_clear();
    command.env("PATH", std::env::var("PATH").unwrap_or_default());

    for (name, value) in environment {
        command.env(name, value);
    }

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
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
    assert_eq!(
        stdout,
        "FILE       VARIABLES  DESCRIPTION\nstats.sql  day        Daily counters\n"
    );
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
                 --quiet \\\n    \
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

#[test]
fn interactive_asks_for_the_unset_variables() {
    let dir = queries_dir("interactive");

    let output = sqlrunner_with_input(
        &["--interactive", "stats.sql"],
        &[
            ("SQLRUNNER_SQL_DIR", dir.to_str().unwrap()),
            ("SQLRUNNER_DSN", UNREACHABLE_DSN),
        ],
        "2026-08-05\n",
    );
    let (stdout, stderr) = streams(&output);

    // The prompt is on stderr, so the command line printed on stdout stays
    // pasteable, and it carries the value that was typed.
    assert!(stderr.starts_with("day: "), "stderr: {stderr}");
    assert!(stdout.contains("-v day=2026-08-05"), "stdout: {stdout}");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn interactive_comes_from_the_environment_too() {
    let dir = queries_dir("interactive-env");

    let output = sqlrunner_with_input(
        &["stats.sql"],
        &[
            ("SQLRUNNER_SQL_DIR", dir.to_str().unwrap()),
            ("SQLRUNNER_DSN", UNREACHABLE_DSN),
            ("SQLRUNNER_INTERACTIVE", "true"),
        ],
        "2026-08-05\n",
    );
    let (stdout, _) = streams(&output);

    assert!(stdout.contains("-v day=2026-08-05"), "stdout: {stdout}");
}

#[test]
fn interactive_without_an_answer_runs_nothing() {
    let dir = queries_dir("interactive-no-answer");

    let output = sqlrunner_with_input(
        &["--interactive", "stats.sql"],
        &[("SQLRUNNER_SQL_DIR", dir.to_str().unwrap())],
        "",
    );
    let (stdout, stderr) = streams(&output);

    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.is_empty(), "stdout: {stdout}");
    assert_eq!(stderr, "day: sqlrunner: stats.sql: day: no value given\n");
}

#[test]
fn completing_prints_one_candidate_per_line() {
    let dir = queries_dir("complete");
    let environment = [("SQLRUNNER_SQL_DIR", dir.to_str().unwrap())];

    let output = sqlrunner(&["--complete", ""], &environment);
    let (stdout, _) = streams(&output);
    assert!(output.status.success());
    assert_eq!(stdout, "stats.sql\n");

    let output = sqlrunner(&["--complete", "stats.sql", ""], &environment);
    let (stdout, _) = streams(&output);
    assert!(output.status.success());
    assert_eq!(stdout, "day=\n");
}

#[test]
fn the_completion_script_needs_no_sql_dir() {
    let output = sqlrunner(&["--completion", "bash"], &[]);
    let (stdout, stderr) = streams(&output);

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stdout.contains("complete -F _sqlrunner sqlrunner"), "stdout: {stdout}");
}
