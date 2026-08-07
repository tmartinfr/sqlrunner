# Completion for sqlrunner, printed by `sqlrunner --completion bash`.
#
# The candidates all come from the binary itself, which knows the .sql files of
# the directory and the variables each of them uses.
_sqlrunner() {
    # Candidates are one per line, as a file name may contain spaces.
    local IFS=$'\n'

    COMPREPLY=($(sqlrunner --complete "${COMP_WORDS[@]:1:COMP_CWORD}"))

    if [ ${#COMPREPLY[@]} -eq 0 ]; then
        # Nothing to offer, as after --sql-dir: fall back to path completion.
        compopt -o default
    elif [[ ${COMPREPLY[0]} == *= ]]; then
        # A variable name is only half a word: the value follows the `=`.
        compopt -o nospace
    fi
}

complete -F _sqlrunner sqlrunner
