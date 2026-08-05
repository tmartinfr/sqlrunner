# sqlrunner

A handy tool for running SQL queries.

## Status

Early stage. `sqlrunner` currently lists the `.sql` files found in a directory
along with the psql-style variables they use, and runs one of them with psql
after printing the command line it uses.

## Requirements

- Rust (edition 2024, tested with cargo 1.93)
- psql in the `PATH`, to run a file

## Install

With `cargo install`, straight from the repository:

```sh
cargo install --git https://github.com/tmartinfr/sqlrunner
```

or from a local clone:

```sh
git clone https://github.com/tmartinfr/sqlrunner
cd sqlrunner
cargo install --path .
```

Either way the binary lands in `~/.cargo/bin/sqlrunner`, which must be in the
`PATH`. Add it if it is not there yet:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

Check the result with:

```sh
sqlrunner --version
```

Re-run the same command to upgrade, and `cargo uninstall sqlrunner` to remove
it.

## Build

To build without installing, from a clone:

```sh
cargo build --release
```

The binary is then `./target/release/sqlrunner`.

## Usage

```
Usage: sqlrunner [OPTIONS] --sql-dir <DIR> [FILE] [NAME=VALUE]...

Arguments:
  [FILE]           File to run with psql, as listed when left out
  [NAME=VALUE]...  Value of a variable used by the file

Options:
      --sql-dir <DIR>  Directory containing the .sql files [env: SQLRUNNER_SQL_DIR]
      --dsn <DSN>      Connection string psql must connect with [env: SQLRUNNER_DSN]
  -h, --help           Print help (see more with '--help')
  -V, --version        Print version
```

There is no subcommand: the file to run is the first argument, and leaving it
out lists the files instead.

### Environment variables

Every option can also be set through an environment variable named
`SQLRUNNER_` followed by the option name in upper case, with `-` turned into
`_`:

| Option      | Variable            |
| ----------- | ------------------- |
| `--sql-dir` | `SQLRUNNER_SQL_DIR` |
| `--dsn`     | `SQLRUNNER_DSN`     |

The command line takes precedence, so a variable acts as a default:

```sh
$ export SQLRUNNER_SQL_DIR=./queries
$ export SQLRUNNER_DSN='postgresql://me@db.example.com/prod'
$ sqlrunner
FILE        VARIABLES
orders.sql  end_date, start_date, status
stats.sql
users.sql   user_id
$ sqlrunner --dsn 'host=localhost dbname=dev' users.sql user_id=42
psql \
    -d 'host=localhost dbname=dev' \
    -v user_id=42 \
    -f ./queries/users.sql

...
```

`--sql-dir` stays mandatory: setting neither the option nor its variable is an
error. The value of `SQLRUNNER_DSN` is kept out of `--help`, as a connection
string may embed a password.

### Listing the files

Without a file argument, list the `.sql` files of a directory as a two-column
table, sorted by base filename, with the psql-style variables each file uses:

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

### Running a file

Pass a file as the first argument to run it with psql, its variables set from
the `NAME=VALUE` arguments that follow. The command line is printed first, one
option per line, then a blank line, then the output of psql itself:

```sh
$ sqlrunner --sql-dir ./queries orders.sql status='in progress' \
    start_date=2026-01-01 end_date=2026-02-01
psql \
    -v end_date=2026-02-01 \
    -v start_date=2026-01-01 \
    -v 'status=in progress' \
    -f ./queries/orders.sql

 id | total
----+-------
  7 | 42.00
(1 row)
```

The command line is displayed in purple, so it stands out from the output of
psql. The escapes are left out when the standard output is not a terminal, so a
redirected or piped run stays plain text.

The file is named as the listing shows it, and must be one of the listed files:
a path is not accepted. Each variable the file uses must be assigned exactly
once, and only those it uses may be assigned. Everything after the first `=` is
the value, so it may itself contain `=` or be empty.

psql is looked up in the `PATH` and inherits the standard streams, so its output
and any prompt it makes reach the terminal unchanged. The printed command line is
quoted for a POSIX shell and continued with `\`, so it can be pasted as is; the
arguments themselves are passed to psql directly, without a shell in between.

`sqlrunner` exits with the exit status of psql: 0 on success, 1 on a fatal psql
error, 2 when the connection fails, 3 on an error in the SQL. A file that could
not be run at all is reported on stderr and exits with status 1, without psql
being started:

```sh
$ sqlrunner --sql-dir ./queries orders.sql start_date=2026-01-01
sqlrunner: orders.sql: unset variables: end_date, status
```

`--dsn` tells psql where to connect, as a `-d` argument. Both forms psql accepts
are passed through untouched, a URI:

```sh
$ sqlrunner --sql-dir ./queries --dsn 'postgresql://me@db.example.com/prod' \
    users.sql user_id=42
psql \
    -d postgresql://me@db.example.com/prod \
    -v user_id=42 \
    -f ./queries/users.sql

...
```

or a keyword/value string:

```sh
$ sqlrunner --sql-dir ./queries --dsn 'host=localhost dbname=prod' \
    users.sql user_id=42
psql \
    -d 'host=localhost dbname=prod' \
    -v user_id=42 \
    -f ./queries/users.sql

...
```

Without `--dsn`, no `-d` is emitted and psql takes its connection settings from
the environment (`PGHOST`, `PGDATABASE`, ...) as usual. `--dsn` is unused when
listing, which reads no database.

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
