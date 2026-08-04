# sqlrunner

A handy tool for running SQL queries.

## Status

Early stage. `sqlrunner` currently lists the `.sql` files found in a directory,
along with the psql-style variables they use.

## Requirements

- Rust (edition 2024, tested with cargo 1.93)

## Build

```sh
cargo build --release
```

## Usage

```
Usage: sqlrunner --sql-dir <DIR>

Options:
      --sql-dir <DIR>  Directory containing the .sql files
  -h, --help           Print help
  -V, --version        Print version
```

List the `.sql` files of a directory, one base filename per line, sorted, each
followed by the psql-style variables it uses:

```sh
$ sqlrunner --sql-dir ./queries
orders.sql: end_date, start_date, status
stats.sql
users.sql: user_id
```

Only regular files directly inside the directory are listed: subdirectories are
not traversed, and files with another extension are ignored. An unreadable or
missing directory is reported on stderr and exits with status 1.

### Variables

A variable is a `:'name'` reference, the psql form that interpolates a value as
a quoted SQL literal:

```sql
SELECT * FROM orders WHERE created_at >= :'start_date';
```

Names are made of ASCII letters, digits and underscores. They are reported
sorted and deduplicated; a file with no variable is listed on its own. Other
psql forms (`:name`, `:"name"`) are not detected.

Occurrences that psql would not interpolate are ignored:

- `--` line comments and `/* */` block comments, including nested ones;
- string literals `'...'`, with `''` and, for `E'...'`, backslash escapes;
- dollar-quoted strings `$$...$$` and `$tag$...$tag$`;
- quoted identifiers `"..."`.

A file that cannot be read is reported on stderr, the other files are still
listed, and the exit status is 1.

## Test

```sh
cargo test
```
