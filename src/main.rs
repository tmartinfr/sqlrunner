use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

/// A handy tool for running SQL queries.
///
/// Without a file, the .sql files of the directory and the variables they use
/// are listed. Each option can also be set through the environment variable
/// named after it, the command line taking precedence.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Directory containing the .sql files
    #[arg(long, value_name = "DIR", env = "SQLRUNNER_SQL_DIR")]
    sql_dir: PathBuf,

    /// Connection string psql must connect with
    // The value is hidden from the help, as a DSN may embed a password.
    #[arg(long, value_name = "DSN", env = "SQLRUNNER_DSN", hide_env_values = true)]
    dsn: Option<String>,

    /// File to run with psql, as listed when left out
    file: Option<String>,

    /// Value of a variable used by the file
    #[arg(value_name = "NAME=VALUE")]
    variables: Vec<String>,
}

/// Returns the `.sql` files directly contained in `dir`, sorted by path.
fn list_sql_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "sql") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Tells whether `byte` can be part of a variable or dollar-quote tag name.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Returns the psql-style variables `:'name'` used in `sql`, sorted and deduplicated.
///
/// A name is made of ASCII alphanumeric characters and underscores; anything
/// else between the quotes is not a variable reference and is skipped.
/// Occurrences inside comments, string literals, dollar-quoted strings and
/// quoted identifiers are ignored, as psql does not interpolate them there.
fn extract_variables(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut variables = BTreeSet::new();
    let mut i = 0;

    while i < bytes.len() {
        let next = bytes.get(i + 1).copied();

        match bytes[i] {
            b':' if next == Some(b'\'') => {
                let start = i + 2;
                let end = start + bytes[start..].iter().take_while(|&&b| is_name_byte(b)).count();

                if end > start && bytes.get(end) == Some(&b'\'') {
                    variables.insert(sql[start..end].to_string());
                    i = end + 1;
                } else {
                    // Not a variable: skip the colon so the quote that follows
                    // is handled as the start of a string literal.
                    i += 1;
                }
            }
            b'-' if next == Some(b'-') => i = skip_line_comment(bytes, i),
            b'/' if next == Some(b'*') => i = skip_block_comment(bytes, i),
            // An `E'...'` literal additionally uses backslash escapes.
            b'\'' => {
                let extended = i >= 1
                    && matches!(bytes[i - 1], b'e' | b'E')
                    && (i < 2 || !is_name_byte(bytes[i - 2]));
                i = skip_quoted(bytes, i, extended);
            }
            b'"' => i = skip_quoted(bytes, i, false),
            b'$' => i = skip_dollar_quoted(bytes, i),
            _ => i += 1,
        }
    }

    variables.into_iter().collect()
}

/// Skips the `--` comment starting at `start`, returning the index of its newline.
fn skip_line_comment(bytes: &[u8], start: usize) -> usize {
    match bytes[start..].iter().position(|&b| b == b'\n') {
        Some(offset) => start + offset,
        None => bytes.len(),
    }
}

/// Skips the `/* */` comment starting at `start`, returning the index just after it.
///
/// Such comments nest in PostgreSQL, so the opening markers are counted.
fn skip_block_comment(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 2;
    let mut depth = 1;

    while i + 1 < bytes.len() {
        match (bytes[i], bytes[i + 1]) {
            (b'/', b'*') => {
                depth += 1;
                i += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }

    bytes.len()
}

/// Skips the string or quoted identifier opened at `start`, returning the index
/// just after its closing quote.
///
/// The quote character is doubled to escape itself; `extended` additionally
/// enables backslash escapes, as in an `E'...'` literal.
fn skip_quoted(bytes: &[u8], start: usize, extended: bool) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;

    while i < bytes.len() {
        if extended && bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] != quote {
            i += 1;
        } else if bytes.get(i + 1) == Some(&quote) {
            i += 2;
        } else {
            return i + 1;
        }
    }

    bytes.len()
}

/// Skips the `$tag$ ... $tag$` string opened at `start`, returning the index
/// just after its closing tag.
///
/// Returns `start + 1` when the `$` does not open one, as in `$1`.
fn skip_dollar_quoted(bytes: &[u8], start: usize) -> usize {
    let tag_end = start + 1 + bytes[start + 1..].iter().take_while(|&&b| is_name_byte(b)).count();

    // A tag is optional but cannot start with a digit, which `$1` does.
    if bytes.get(tag_end) != Some(&b'$') || bytes[start + 1].is_ascii_digit() {
        return start + 1;
    }

    let tag = &bytes[start..=tag_end];
    let body = tag_end + 1;

    match bytes[body..].windows(tag.len()).position(|w| w == tag) {
        Some(offset) => body + offset + tag.len(),
        None => bytes.len(),
    }
}

/// Renders `rows` as a two-column table, the first column padded to a common width.
///
/// The header is included, and an empty second column leaves no trailing space.
fn format_table(headers: (&str, &str), rows: &[(String, String)]) -> String {
    let width = rows
        .iter()
        .map(|(left, _)| left.chars().count())
        .chain(std::iter::once(headers.0.chars().count()))
        .max()
        .unwrap_or(0);

    let mut table = String::new();

    for (left, right) in std::iter::once((headers.0.to_string(), headers.1.to_string()))
        .chain(rows.iter().cloned())
    {
        if right.is_empty() {
            table.push_str(&left);
        } else {
            let padding = " ".repeat(width - left.chars().count());
            table.push_str(&format!("{left}{padding}  {right}"));
        }
        table.push('\n');
    }

    table
}

/// Splits a `name=value` argument, the value being allowed to contain `=`.
fn parse_assignment(argument: &str) -> Result<(String, String), String> {
    match argument.split_once('=') {
        Some((name, _)) if name.is_empty() => {
            Err(format!("{argument}: missing variable name"))
        }
        Some((name, value)) => Ok((name.to_string(), value.to_string())),
        None => Err(format!("{argument}: expected NAME=VALUE")),
    }
}

/// Quotes `argument` for a POSIX shell, leaving an already safe one untouched.
fn shell_quote(argument: &str) -> String {
    let is_safe = |c: char| c.is_ascii_alphanumeric() || "-_./=:,+@".contains(c);

    if !argument.is_empty() && argument.chars().all(is_safe) {
        argument.to_string()
    } else {
        format!("'{}'", argument.replace('\'', r"'\''"))
    }
}

/// Returns the psql command running `file` with `variables` set, as an argument
/// vector starting with the program name.
///
/// Without a `dsn`, psql takes its connection settings from the environment.
fn psql_command(
    file: &Path,
    dsn: Option<&str>,
    variables: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut command = vec!["psql".to_string()];

    if let Some(dsn) = dsn {
        command.push("-d".to_string());
        command.push(dsn.to_string());
    }

    for (name, value) in variables {
        command.push("-v".to_string());
        command.push(format!("{name}={value}"));
    }

    command.push("-f".to_string());
    command.push(file.to_string_lossy().into_owned());
    command
}

/// Renders `command` as a command line a POSIX shell would run identically,
/// the program on the first line then one option per continuation line.
///
/// An option is a flag and the value that follows it, as psql takes them.
fn format_command(command: &[String]) -> String {
    let mut lines = vec![shell_quote(&command[0])];

    for option in command[1..].chunks(2) {
        let arguments: Vec<String> = option.iter().map(|argument| shell_quote(argument)).collect();
        lines.push(format!("    {}", arguments.join(" ")));
    }

    lines.join(" \\\n")
}

/// Wraps `text` in the escapes displaying it in purple.
fn purple(text: &str) -> String {
    format!("\x1b[35m{text}\x1b[0m")
}

/// Displays `text` in purple, plain when the standard output is not a terminal
/// able to interpret the escapes.
fn colorize(text: &str) -> String {
    if io::IsTerminal::is_terminal(&io::stdout()) {
        purple(text)
    } else {
        text.to_string()
    }
}

/// Runs `command`, letting it inherit the standard streams, and returns its
/// exit status as an exit code.
///
/// A status without a code, as when a signal terminates psql, is a failure.
fn execute(command: &[String]) -> Result<ExitCode, String> {
    // Our own output must reach the terminal before psql writes to it.
    io::Write::flush(&mut io::stdout()).map_err(|err| format!("stdout: {err}"))?;

    let status = std::process::Command::new(&command[0])
        .args(&command[1..])
        .status()
        .map_err(|err| format!("{}: {err}", command[0]))?;

    match status.code() {
        Some(0) => Ok(ExitCode::SUCCESS),
        Some(code) => Ok(ExitCode::from(code as u8)),
        None => Ok(ExitCode::FAILURE),
    }
}

/// Returns the psql command running the `file` of `dir` with `assignments`.
///
/// The file must be one of those listed, and the assignments must cover exactly
/// the variables it uses.
fn run(
    dir: &Path,
    dsn: Option<&str>,
    file: &str,
    assignments: &[String],
) -> Result<Vec<String>, String> {
    let files = list_sql_files(dir).map_err(|err| format!("{}: {err}", dir.display()))?;

    let path = files
        .iter()
        .find(|path| path.file_name().is_some_and(|name| name == file))
        .ok_or_else(|| format!("{file}: no such .sql file in {}", dir.display()))?;

    let sql = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let used: BTreeSet<String> = extract_variables(&sql).into_iter().collect();
    let mut variables = BTreeMap::new();

    for assignment in assignments {
        let (name, value) = parse_assignment(assignment)?;

        if !used.contains(&name) {
            return Err(format!("{file}: unused variable: {name}"));
        }
        if variables.insert(name.clone(), value).is_some() {
            return Err(format!("{file}: variable set twice: {name}"));
        }
    }

    let missing: Vec<&str> = used
        .iter()
        .filter(|name| !variables.contains_key(*name))
        .map(String::as_str)
        .collect();

    if !missing.is_empty() {
        return Err(format!("{file}: unset variables: {}", missing.join(", ")));
    }

    Ok(psql_command(path, dsn, &variables))
}

/// Prints the `.sql` files of `dir` and the variables they use.
fn list(dir: &Path) -> ExitCode {
    let files = match list_sql_files(dir) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("sqlrunner: {}: {}", dir.display(), err);
            return ExitCode::FAILURE;
        }
    };

    let mut exit_code = ExitCode::SUCCESS;
    let mut rows = Vec::new();

    for file in files {
        match std::fs::read_to_string(&file) {
            Ok(sql) => {
                // `file_name` is always set: only paths with a `.sql` extension are listed.
                let name = file.file_name().unwrap_or_default().to_string_lossy();
                rows.push((name.into_owned(), extract_variables(&sql).join(", ")));
            }
            Err(err) => {
                eprintln!("sqlrunner: {}: {}", file.display(), err);
                exit_code = ExitCode::FAILURE;
            }
        }
    }

    if !rows.is_empty() {
        print!("{}", format_table(("FILE", "VARIABLES"), &rows));
    }

    exit_code
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let Some(file) = cli.file else {
        return list(&cli.sql_dir);
    };

    let command = match run(&cli.sql_dir, cli.dsn.as_deref(), &file, &cli.variables) {
        Ok(command) => command,
        Err(err) => {
            eprintln!("sqlrunner: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("{}", colorize(&format_command(&command)));
    println!();

    match execute(&command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("sqlrunner: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates an empty directory dedicated to a single test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sqlrunner-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lists_only_sql_files_sorted() {
        let dir = temp_dir("lists-only-sql-files-sorted");
        std::fs::write(dir.join("b.sql"), "").unwrap();
        std::fs::write(dir.join("a.sql"), "").unwrap();
        std::fs::write(dir.join("notes.txt"), "").unwrap();
        std::fs::write(dir.join("no-extension"), "").unwrap();
        std::fs::create_dir(dir.join("sub.sql")).unwrap();

        let files = list_sql_files(&dir).unwrap();

        assert_eq!(files, vec![dir.join("a.sql"), dir.join("b.sql")]);
    }

    #[test]
    fn empty_directory_yields_no_file() {
        let dir = temp_dir("empty-directory-yields-no-file");

        assert!(list_sql_files(&dir).unwrap().is_empty());
    }

    #[test]
    fn missing_directory_is_an_error() {
        let dir = temp_dir("missing-directory-is-an-error");
        std::fs::remove_dir_all(&dir).unwrap();

        assert!(list_sql_files(&dir).is_err());
    }

    #[test]
    fn extracts_variables_sorted_and_deduplicated() {
        let sql = "SELECT * FROM t \
                   WHERE day >= :'start_date' AND day < :'end_date' \
                     AND owner = :'owner1' AND creator = :'owner1';";

        assert_eq!(
            extract_variables(sql),
            vec!["end_date", "owner1", "start_date"]
        );
    }

    #[test]
    fn ignores_non_variables() {
        // No variable: a plain literal, a cast, an unquoted psql variable, an
        // empty name, a name with an invalid character, and an unterminated quote.
        let sql = "SELECT 'a', x::text, :plain, :'', :'not a name', :'unterminated";

        assert!(extract_variables(sql).is_empty());
    }

    #[test]
    fn extracts_no_variable_from_plain_sql() {
        assert!(extract_variables("SELECT 1;").is_empty());
    }

    #[test]
    fn ignores_variables_inside_comments() {
        let sql = "-- see :'commented_out'\n\
                   /* :'block' /* :'nested' */ still a comment :'deep' */\n\
                   SELECT :'kept'; -- :'trailing'";

        assert_eq!(extract_variables(sql), vec!["kept"]);
    }

    #[test]
    fn ignores_variables_inside_literals_and_identifiers() {
        let sql = "SELECT ':''not_a_var''', \"col :'not_a_var'\", \
                   E'escaped \\' :''not_a_var''', \
                   $body$ :'not_a_var' $body$, $1, :'kept';";

        assert_eq!(extract_variables(sql), vec!["kept"]);
    }

    #[test]
    fn keeps_scanning_after_a_doubled_quote_in_a_literal() {
        let sql = "SELECT 'it''s :''hidden'' here', :'kept';";

        assert_eq!(extract_variables(sql), vec!["kept"]);
    }

    #[test]
    fn formats_a_two_column_table() {
        let rows = vec![
            ("comments.sql".to_string(), "owner, start_date".to_string()),
            ("stats.sql".to_string(), String::new()),
        ];

        assert_eq!(
            format_table(("FILE", "VARIABLES"), &rows),
            "FILE          VARIABLES\n\
             comments.sql  owner, start_date\n\
             stats.sql\n"
        );
    }

    #[test]
    fn table_column_is_at_least_as_wide_as_its_header() {
        let rows = vec![("a.sql".to_string(), "v".to_string())];

        assert_eq!(
            format_table(("FILE", "VARIABLES"), &rows),
            "FILE   VARIABLES\n\
             a.sql  v\n"
        );
    }

    #[test]
    fn parses_assignments() {
        assert_eq!(
            parse_assignment("day=2026-08-04").unwrap(),
            ("day".to_string(), "2026-08-04".to_string())
        );
        // The value keeps everything after the first `=`, and may be empty.
        assert_eq!(
            parse_assignment("filter=a=b").unwrap(),
            ("filter".to_string(), "a=b".to_string())
        );
        assert_eq!(
            parse_assignment("empty=").unwrap(),
            ("empty".to_string(), String::new())
        );
        assert!(parse_assignment("day").is_err());
        assert!(parse_assignment("=value").is_err());
    }

    #[test]
    fn quotes_only_unsafe_arguments() {
        assert_eq!(shell_quote("day=2026-08-04"), "day=2026-08-04");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("name=a b"), "'name=a b'");
        assert_eq!(shell_quote("name=it's"), r"'name=it'\''s'");
    }

    #[test]
    fn runs_a_file_with_its_variables_set() {
        let dir = temp_dir("runs-a-file-with-its-variables-set");
        let sql = "SELECT * FROM t WHERE day >= :'start' AND owner = :'owner';";
        std::fs::write(dir.join("orders.sql"), sql).unwrap();

        let command = run(
            &dir,
            None,
            "orders.sql",
            &["owner=a's shop".to_string(), "start=2026-08-04".to_string()],
        )
        .unwrap();

        // The arguments are passed to psql unquoted, the quoting being only a
        // matter of rendering the command line.
        assert_eq!(
            command,
            vec![
                "psql",
                "-v",
                "owner=a's shop",
                "-v",
                "start=2026-08-04",
                "-f",
                &dir.join("orders.sql").to_string_lossy(),
            ]
        );
        assert_eq!(
            format_command(&command),
            format!(
                "psql \\\n    \
                     -v 'owner=a'\\''s shop' \\\n    \
                     -v start=2026-08-04 \\\n    \
                     -f {}",
                dir.join("orders.sql").display()
            )
        );
    }

    #[test]
    fn runs_a_file_against_a_dsn() {
        let dir = temp_dir("runs-a-file-against-a-dsn");
        std::fs::write(dir.join("stats.sql"), "SELECT :'day';").unwrap();

        let dsn = "postgresql://user@host:5432/db?sslmode=require";
        let command = run(&dir, Some(dsn), "stats.sql", &["day=2026-08-04".into()]).unwrap();

        // The `?` of the query string is quoted, as a shell would expand it.
        assert_eq!(
            format_command(&command),
            format!(
                "psql \\\n    \
                     -d '{dsn}' \\\n    \
                     -v day=2026-08-04 \\\n    \
                     -f {}",
                dir.join("stats.sql").display()
            )
        );
    }

    #[test]
    fn quotes_a_dsn_needing_it() {
        let dir = temp_dir("quotes-a-dsn-needing-it");
        std::fs::write(dir.join("stats.sql"), "SELECT 1;").unwrap();

        let command = run(&dir, Some("host=localhost dbname=db"), "stats.sql", &[]).unwrap();

        assert_eq!(
            format_command(&command),
            format!(
                "psql \\\n    \
                     -d 'host=localhost dbname=db' \\\n    \
                     -f {}",
                dir.join("stats.sql").display()
            )
        );
    }

    #[test]
    fn runs_a_file_without_variable() {
        let dir = temp_dir("runs-a-file-without-variable");
        std::fs::write(dir.join("stats.sql"), "SELECT 1;").unwrap();

        assert_eq!(
            format_command(&run(&dir, None, "stats.sql", &[]).unwrap()),
            format!("psql \\\n    -f {}", dir.join("stats.sql").display())
        );
    }

    #[test]
    fn running_rejects_an_invalid_invocation() {
        let dir = temp_dir("running-rejects-an-invalid-invocation");
        std::fs::write(dir.join("orders.sql"), "SELECT :'start';").unwrap();

        // An unknown file, an unset variable, an unused one, and a duplicate.
        assert!(run(&dir, None, "missing.sql", &[]).is_err());
        assert!(run(&dir, None, "orders.sql", &[]).is_err());
        assert!(run(&dir, None, "orders.sql", &["start=1".into(), "other=2".into()]).is_err());
        assert!(run(&dir, None, "orders.sql", &["start=1".into(), "start=2".into()]).is_err());
    }

    #[test]
    fn running_ignores_a_file_outside_the_directory() {
        let dir = temp_dir("running-ignores-a-file-outside-the-directory");
        std::fs::create_dir(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/orders.sql"), "SELECT 1;").unwrap();

        assert!(run(&dir, None, "sub/orders.sql", &[]).is_err());
        assert!(run(&dir, None, "../orders.sql", &[]).is_err());
    }

    #[test]
    fn colors_in_purple() {
        assert_eq!(purple("psql \\\n    -f a.sql"), "\x1b[35mpsql \\\n    -f a.sql\x1b[0m");
        // The tests capture the standard output, so no escape is emitted.
        assert_eq!(colorize("psql"), "psql");
    }

    #[test]
    fn executing_reports_a_missing_program() {
        let err = execute(&["sqlrunner-no-such-program".to_string()]).unwrap_err();

        assert!(err.starts_with("sqlrunner-no-such-program: "));
    }

    #[test]
    fn executing_returns_the_exit_code_of_the_program() {
        // `false` is the shortest program with a non-zero, non-signal status.
        assert_eq!(
            format!("{:?}", execute(&["false".to_string()]).unwrap()),
            format!("{:?}", ExitCode::from(1))
        );
        assert_eq!(
            format!("{:?}", execute(&["true".to_string()]).unwrap()),
            format!("{:?}", ExitCode::SUCCESS)
        );
    }

    #[test]
    fn options_read_their_environment_variable() {
        use clap::CommandFactory;

        let command = Cli::command();
        let variables: Vec<(&str, Option<&str>)> = command
            .get_arguments()
            .filter(|arg| !matches!(arg.get_id().as_str(), "help" | "version"))
            .filter(|arg| arg.get_long().is_some())
            .map(|arg| (arg.get_id().as_str(), arg.get_env().and_then(|env| env.to_str())))
            .collect();

        assert_eq!(
            variables,
            vec![
                ("sql_dir", Some("SQLRUNNER_SQL_DIR")),
                ("dsn", Some("SQLRUNNER_DSN")),
            ]
        );
    }

    #[test]
    fn unterminated_literals_and_comments_do_not_loop() {
        assert!(extract_variables("SELECT 'oops :'x'").is_empty());
        assert!(extract_variables("/* oops :'x'").is_empty());
        assert!(extract_variables("SELECT $tag$ oops :'x'").is_empty());
        assert!(extract_variables("SELECT $").is_empty());
    }
}
