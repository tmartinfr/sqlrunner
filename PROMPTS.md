Init rust project named "sqlrunner" which is a handy tool for running SQL queries

List files with extension `.sql` that are present in a directory given by `--sql-dir` option

Display only the base filename

Extract the psql-style variables (:'variable') that could be present in files and display them along with the filename

Exclude matches inside literals or comments

Arrange output in a two-colums table

Add "run" sub command who takes one of the available SQL files as first argument, then some key=value options. Output a valid psql command line that run the SQL file with its variables set from the key-value options. "--sql-dir" should remain a top level option.

Add top level --dsn option to tell sqlrunner where psql must connect

Allow to set top level options as environment variables named SQLRUNNER_OPTIONNAME

Add a delimiter after displaying the psql command, then actually run it and display the output

Split the command line display in multiple lines, one line per option with a 4-spaces indentation. Display it in purple color. Replace the delimiter with a blank line.

Add README instructions on how to install

Remove the run subcommand, the SQL file can be directly passed as first argument

Add a third column to the table listing with an optional description who have to appear as a SQL comment on the first line of a SQL file

Add --interactive top level option. If set, ask for missing variables.

Add automatic completion on SQL filename and variables

