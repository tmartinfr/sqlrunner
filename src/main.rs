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

fn main() -> ExitCode {
    let cli = Cli::parse();

    match list_sql_files(&cli.sql_dir) {
        Ok(files) => {
            for file in files {
                // `file_name` is always set: only paths with a `.sql` extension are listed.
                println!("{}", file.file_name().unwrap_or_default().display());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("sqlrunner: {}: {}", cli.sql_dir.display(), err);
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
}
