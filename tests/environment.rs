//! Checks that the top-level options can be set through the environment.
//!
//! The binary is run as a child process, so the variables are set on it alone.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Creates a directory holding a single `stats.sql` file using one variable.
fn queries_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sqlrunner-env-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("stats.sql"), "SELECT :'day';").unwrap();
    dir
}

/// Runs the binary with `arguments` and the given environment variables only.
fn sqlrunner(arguments: &[&str], environment: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sqlrunner"));
    command.args(arguments).env_clear();

    for (name, value) in environment {
        command.env(name, value);
    }

    command.output().unwrap()
}

#[test]
fn sql_dir_comes_from_the_environment() {
    let dir = queries_dir("sql-dir");

    let output = sqlrunner(&[], &[("SQLRUNNER_SQL_DIR", &dir)]);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "FILE       VARIABLES\nstats.sql  day\n"
    );
}

#[test]
fn dsn_comes_from_the_environment() {
    let dir = queries_dir("dsn");

    let output = sqlrunner(
        &["run", "stats.sql", "day=2026-08-05"],
        &[
            ("SQLRUNNER_SQL_DIR", &dir),
            ("SQLRUNNER_DSN", Path::new("dbname=from_env")),
        ],
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "psql -d dbname=from_env -v day=2026-08-05 -f {}\n",
            dir.join("stats.sql").display()
        )
    );
}

#[test]
fn the_command_line_wins_over_the_environment() {
    let dir = queries_dir("precedence");

    let output = sqlrunner(
        &[
            "--sql-dir",
            dir.to_str().unwrap(),
            "--dsn",
            "dbname=from_args",
            "run",
            "stats.sql",
            "day=2026-08-05",
        ],
        &[
            ("SQLRUNNER_SQL_DIR", Path::new("/does/not/exist")),
            ("SQLRUNNER_DSN", Path::new("dbname=from_env")),
        ],
    );

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "psql -d dbname=from_args -v day=2026-08-05 -f {}\n",
            dir.join("stats.sql").display()
        )
    );
}

#[test]
fn a_missing_sql_dir_is_still_required() {
    let output = sqlrunner(&[], &[]);

    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr).unwrap().contains("--sql-dir"));
}
