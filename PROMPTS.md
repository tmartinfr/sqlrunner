Init rust project named "sqlrunner" which is a handy tool for running SQL queries
List files with extension `.sql` that are present in a directory given by `--sql-dir` option
Display only the base filename
Extract the psql-style variables (:'variable') that could be present in files and display them along with the filename
Exclude matches inside literals or comments
Arrange output in a two-colums table
Add "run" sub command who takes one of the available SQL files as first argument, then some key=value options. Output a valid psql command line that run the SQL file with its variables set from the key-value options. "--sql-dir" should remain a top level option.
Add top level --dsn option to tell sqlrunner where psql must connect
Allow to set top level options as environment variables named SQLRUNNER_OPTIONNAME
