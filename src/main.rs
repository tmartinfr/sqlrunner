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

/// Returns the psql-style variables `:'name'` used in `sql`, sorted and deduplicated.
///
/// A name is made of ASCII alphanumeric characters and underscores; anything
/// else between the quotes is not a variable reference and is skipped.
fn extract_variables(sql: &str) -> Vec<String> {
    let mut variables = BTreeSet::new();
    let mut rest = sql;

    while let Some(start) = rest.find(":'") {
        rest = &rest[start + 2..];

        let Some(end) = rest.find('\'') else { break };
        let (name, after) = (&rest[..end], &rest[end + 1..]);

        if !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            variables.insert(name.to_string());
        }

        rest = after;
    }

    variables.into_iter().collect()
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

    for file in files {
        // `file_name` is always set: only paths with a `.sql` extension are listed.
        let name = file.file_name().unwrap_or_default().display();

        match std::fs::read_to_string(&file) {
            Ok(sql) => {
                let variables = extract_variables(&sql);
                if variables.is_empty() {
                    println!("{name}");
                } else {
                    println!("{name}: {}", variables.join(", "));
                }
            }
            Err(err) => {
                eprintln!("sqlrunner: {}: {}", file.display(), err);
                exit_code = ExitCode::FAILURE;
            }
        }
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
}
