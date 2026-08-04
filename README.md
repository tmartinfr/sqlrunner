# sqlrunner

A handy tool for running SQL queries.

## Status

Early stage. `sqlrunner` currently lists the `.sql` files found in a directory.

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

List the `.sql` files of a directory, one base filename per line, sorted:

```sh
$ sqlrunner --sql-dir ./queries
orders.sql
users.sql
```

Only regular files directly inside the directory are listed: subdirectories are
not traversed, and files with another extension are ignored. An unreadable or
missing directory is reported on stderr and exits with status 1.

## Test

```sh
cargo test
```
