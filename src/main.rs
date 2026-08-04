use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

/// A handy tool for running SQL queries.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Directory containing the .sql files
    #[arg(long, value_name = "DIR")]
    sql_dir: PathBuf,
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

fn main() -> ExitCode {
    let cli = Cli::parse();

    let files = match list_sql_files(&cli.sql_dir) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("sqlrunner: {}: {}", cli.sql_dir.display(), err);
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
    fn unterminated_literals_and_comments_do_not_loop() {
        assert!(extract_variables("SELECT 'oops :'x'").is_empty());
        assert!(extract_variables("/* oops :'x'").is_empty());
        assert!(extract_variables("SELECT $tag$ oops :'x'").is_empty());
        assert!(extract_variables("SELECT $").is_empty());
    }
}
