# sqlrunner

A handy tool for running SQL queries.

## Status

Early stage. `sqlrunner` currently lists the `.sql` files found in a directory
along with the psql-style variables they use, and prints the psql command line
running one of them. It does not run anything itself yet.

## Requirements

- Rust (edition 2024, tested with cargo 1.93)

## Build

```sh
cargo build --release
```

## Usage

```
Usage: sqlrunner --sql-dir <DIR> [COMMAND]

Commands:
  list  List the .sql files and the variables they use (default)
  run   Print the psql command line running a .sql file
  help  Print this message or the help of the given subcommand(s)

Options:
      --sql-dir <DIR>  Directory containing the .sql files
  -h, --help           Print help
  -V, --version        Print version
```

`--sql-dir` is a top-level option and comes before the subcommand.

### list

List the `.sql` files of a directory as a two-column table, sorted by base
filename, with the psql-style variables each file uses:

```sh
$ sqlrunner --sql-dir ./queries
FILE        VARIABLES
orders.sql  end_date, start_date, status
stats.sql
users.sql   user_id
```

Only regular files directly inside the directory are listed: subdirectories are
not traversed, and files with another extension are ignored. An unreadable or
missing directory is reported on stderr and exits with status 1.

This is the default command: `sqlrunner --sql-dir ./queries` and
`sqlrunner --sql-dir ./queries list` are equivalent.

### run

Print, without running it, the psql command line that runs one file with its
variables set from `NAME=VALUE` arguments:

```sh
$ sqlrunner --sql-dir ./queries run orders.sql status='in progress' \
    start_date=2026-01-01 end_date=2026-02-01
psql -v end_date=2026-02-01 -v start_date=2026-01-01 -v 'status=in progress' -f ./queries/orders.sql
```

The file is named as `list` shows it, and must be one of the listed files: a
path is not accepted. Each variable the file uses must be assigned exactly once,
and only those it uses may be assigned. Everything after the first `=` is the
value, so it may itself contain `=` or be empty.

Values are quoted for a POSIX shell, so the output can be evaluated as is. The
resulting `psql` call takes its connection settings from the environment
(`PGHOST`, `PGDATABASE`, ...) as usual.

A bad invocation is reported on stderr and exits with status 1:

```sh
$ sqlrunner --sql-dir ./queries run orders.sql start_date=2026-01-01
sqlrunner: orders.sql: unset variables: end_date, status
```

### Variables

A variable is a `:'name'` reference, the psql form that interpolates a value as
a quoted SQL literal:

```sql
SELECT * FROM orders WHERE created_at >= :'start_date';
```

Names are made of ASCII letters, digits and underscores. They are reported
sorted and deduplicated; a file with no variable has an empty second column.
Other
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
