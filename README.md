# sqlrunner

A handy tool for running SQL queries.

## Status

Early stage. `sqlrunner` currently lists the `.sql` files found in a directory
along with the psql-style variables they use and their description, and runs one
of them with psql after printing the command line it uses, asking for the
variables left unset when told to. bash and zsh can complete the file and its
variables.

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

## Example

The `example` directory holds a few `.sql` files to try the tool on, and
`example.schema.sql` creates the tables they read, with a few rows:

```sh
psql -d 'host=localhost dbname=dev' -f example.schema.sql
sqlrunner --sql-dir ./example
```

`orders.sql`, `stats.sql` and `users.sql` are plain queries, with and without
variables. `audit.sql` shows what is not a variable: it holds `:'name'`
lookalikes in comments, string literals, a dollar-quoted string and a quoted
identifier, none of which appear among its variables. Every example below runs
on that directory.

## Usage

```
Usage: sqlrunner [OPTIONS] [FILE] [NAME=VALUE]...

Arguments:
  [FILE]           File to run with psql, as listed when left out
  [NAME=VALUE]...  Value of a variable used by the file

Options:
      --sql-dir <DIR>       Directory containing the .sql files [env: SQLRUNNER_SQL_DIR]
      --dsn <DSN>           Connection string psql must connect with [env: SQLRUNNER_DSN]
  -i, --interactive         Ask for the variables left unset instead of failing [env: SQLRUNNER_INTERACTIVE]
      --completion <SHELL>  Print the completion script to source for a shell [possible values: bash, zsh]
  -h, --help                Print help (see more with '--help')
  -V, --version             Print version
```

There is no subcommand: the file to run is the first argument, and leaving it
out lists the files instead.

### Environment variables

Every option can also be set through an environment variable named
`SQLRUNNER_` followed by the option name in upper case, with `-` turned into
`_`:

| Option          | Variable                |
| --------------- | ----------------------- |
| `--sql-dir`     | `SQLRUNNER_SQL_DIR`     |
| `--dsn`         | `SQLRUNNER_DSN`         |
| `--interactive` | `SQLRUNNER_INTERACTIVE` |

The command line takes precedence, so a variable acts as a default:

```sh
$ export SQLRUNNER_SQL_DIR=./example
$ export SQLRUNNER_DSN='postgresql://me@db.example.com/prod'
$ sqlrunner
FILE        VARIABLES                      DESCRIPTION
audit.sql   action, row_limit, table_name  Audit trail of a table, ignoring quoted lookalikes
orders.sql  end_date, start_date, status   Orders of a period, by status
stats.sql                                  User counts, total and last 30 days
users.sql   user_id                        Details of one user
$ sqlrunner --dsn 'host=localhost dbname=dev' users.sql user_id=42
psql --quiet \
    -d 'host=localhost dbname=dev' \
    -v user_id=42 \
    -f ./example/users.sql

...
```

`--sql-dir` stays mandatory, except with `--completion`: setting neither the
option nor its variable is an error. The value of `SQLRUNNER_DSN` is kept out of
`--help`, as a connection string may embed a password.

### Listing the files

Without a file argument, list the `.sql` files of a directory as a three-column
table, sorted by base filename, with the psql-style variables each file uses and
its description:

```sh
$ sqlrunner --sql-dir ./example
FILE        VARIABLES                      DESCRIPTION
audit.sql   action, row_limit, table_name  Audit trail of a table, ignoring quoted lookalikes
orders.sql  end_date, start_date, status   Orders of a period, by status
stats.sql                                  User counts, total and last 30 days
users.sql   user_id                        Details of one user
```

Only regular files directly inside the directory are listed: subdirectories are
not traversed, and files with another extension are ignored. An unreadable or
missing directory is reported on stderr and exits with status 1.

### Running a file

Pass a file as the first argument to run it with psql, its variables set from
the `NAME=VALUE` arguments that follow. The command line is printed first, one
option per line, then a blank line, then the output of psql itself:

```sh
$ sqlrunner --sql-dir ./example orders.sql status='in progress' \
    start_date=2026-01-01 end_date=2026-02-01
psql --quiet \
    -v end_date=2026-02-01 \
    -v start_date=2026-01-01 \
    -v 'status=in progress' \
    -f ./example/orders.sql

 id |   status    | total
----+-------------+--------
  1 | in progress |  42.00
  3 | in progress | 128.90
(2 rows)
```

The command line is displayed in purple, so it stands out from the output of
psql. The escapes are left out when the standard output is not a terminal, so a
redirected or piped run stays plain text.

The file is named as the listing shows it, and must be one of the listed files:
a path is not accepted. Each variable the file uses must be assigned exactly
once, and only those it uses may be assigned. Everything after the first `=` is
the value, so it may itself contain `=` or be empty.

psql is always run with `--quiet`, so its output holds nothing but what the file
itself produces, without the welcome banner or the command tag of each statement.

psql is looked up in the `PATH` and inherits the standard streams, so its output
and any prompt it makes reach the terminal unchanged. The printed command line is
quoted for a POSIX shell and continued with `\`, so it can be pasted as is; the
arguments themselves are passed to psql directly, without a shell in between.

`sqlrunner` exits with the exit status of psql: 0 on success, 1 on a fatal psql
error, 2 when the connection fails, 3 on an error in the SQL. A file that could
not be run at all is reported on stderr and exits with status 1, without psql
being started:

```sh
$ sqlrunner --sql-dir ./example orders.sql start_date=2026-01-01
sqlrunner: orders.sql: unset variables: end_date, status
```

`--dsn` tells psql where to connect, as a `-d` argument. Both forms psql accepts
are passed through untouched, a URI:

```sh
$ sqlrunner --sql-dir ./example --dsn 'postgresql://me@db.example.com/prod' \
    users.sql user_id=42
psql --quiet \
    -d postgresql://me@db.example.com/prod \
    -v user_id=42 \
    -f ./example/users.sql

...
```

or a keyword/value string:

```sh
$ sqlrunner --sql-dir ./example --dsn 'host=localhost dbname=prod' \
    users.sql user_id=42
psql --quiet \
    -d 'host=localhost dbname=prod' \
    -v user_id=42 \
    -f ./example/users.sql

...
```

Without `--dsn`, no `-d` is emitted and psql takes its connection settings from
the environment (`PGHOST`, `PGDATABASE`, ...) as usual. `--dsn` is unused when
listing, which reads no database.

### Asking for the variables

With `--interactive` (`-i`), a variable the command line leaves unset is asked
for instead of being an error. The variables are asked for in the order the
listing shows them, one line each, and the values already given are not asked
for again:

```sh
$ sqlrunner --sql-dir ./example --interactive orders.sql status='in progress'
end_date: 2026-02-01
start_date: 2026-01-01
psql --quiet \
    -v end_date=2026-02-01 \
    -v start_date=2026-01-01 \
    -v 'status=in progress' \
    -f ./example/orders.sql

...
```

The prompts go to stderr, so that the standard output holds nothing but the
command line and the output of psql. An answer is taken as typed, spaces
included, and an empty line sets an empty value. Input ending before a value is
given, as when reading from a closed or empty standard input, is an error and
psql is not started:

```sh
$ sqlrunner --sql-dir ./example --interactive orders.sql < /dev/null
end_date: sqlrunner: orders.sql: end_date: no value given
```

Without `--interactive`, an unset variable is still the error described above.
`--interactive` is unused when listing, which needs no variable.

### Variables

A variable is a `:'name'` reference, the psql form that interpolates a value as
a quoted SQL literal:

```sql
SELECT * FROM orders WHERE created_at >= :'start_date';
```

Names are made of ASCII letters, digits and underscores. They are reported
sorted and deduplicated; a file with no variable has an empty `VARIABLES` cell.
Other psql forms (`:name`, `:"name"`) are not detected.

Occurrences that psql would not interpolate are ignored:

- `--` line comments and `/* */` block comments, including nested ones;
- string literals `'...'`, with `''` and, for `E'...'`, backslash escapes;
- dollar-quoted strings `$$...$$` and `$tag$...$tag$`;
- quoted identifiers `"..."`.

A file that cannot be read is reported on stderr, the other files are still
listed, and the exit status is 1.

### Descriptions

A description is optional: it is the text of a `--` comment making up the first
line of a file, and says in a few words what the file does.

```sql
-- Orders of a period, by status
SELECT * FROM orders WHERE created_at >= :'start_date';
```

The `--` marker and the spaces around the text are left out. A file whose first
line is not such a comment, or whose comment holds nothing but spaces, has an
empty `DESCRIPTION` cell. Only the first line is looked at: a comment further
down the file, or one trailing a statement, is not a description.

## Completion

`sqlrunner --completion <SHELL>` prints the completion script of a shell, bash
or zsh. Source it from the shell startup file:

```sh
# ~/.bashrc
source <(sqlrunner --completion bash)
```

```sh
# ~/.zshrc, after compinit has run
source <(sqlrunner --completion zsh)
```

The candidates are computed by `sqlrunner` itself, which the script calls, so
they always match the directory in use:

```sh
$ sqlrunner <TAB>
audit.sql  orders.sql  stats.sql  users.sql
$ sqlrunner orders.sql <TAB>
end_date=  start_date=  status=
$ sqlrunner orders.sql status=paid <TAB>
end_date=  start_date=
```

A variable is completed up to its `=`, with no space after it, and the names
already given are left out. The file completed on is the one the directory
holds, `--sql-dir` on the command line taking precedence over
`SQLRUNNER_SQL_DIR`, as when running. A word starting with `-` completes to the
options, and `--sql-dir` falls back to the path completion of the shell.

The script calls `sqlrunner --complete` with the words typed so far. That option
is internal, prints one candidate per line, and is of no use by hand.

## Test

```sh
cargo test
```
