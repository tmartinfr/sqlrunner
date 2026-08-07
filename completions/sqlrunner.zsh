# Completion for sqlrunner, printed by `sqlrunner --completion zsh`.
#
# The candidates all come from the binary itself, which knows the .sql files of
# the directory and the variables each of them uses.
_sqlrunner() {
    local -a candidates

    # The words from the first argument to the one being completed, which is
    # empty when the cursor sits on a fresh word.
    candidates=(${(f)"$(sqlrunner --complete "${(@)words[2,CURRENT]}")"})

    if (( ${#candidates} == 0 )); then
        # Nothing to offer, as after --sql-dir: fall back to path completion.
        _files
    elif [[ ${candidates[1]} == *= ]]; then
        # A variable name is only half a word: the value follows the `=`.
        compadd -S '' -- ${candidates}
    else
        compadd -- ${candidates}
    fi
}

compdef _sqlrunner sqlrunner
