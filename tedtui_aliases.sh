alias tedt='eval "tedtui $(fted)"' 
alias tt=tedtui
alias tedtrash='cd ~/.ted/todos && fzf --bind "enter:execute-silent(mv {} ~/.ted/trash/)+reload(find . -name \"*.md\" -type f)" --preview "cat {}" --header "Enter=trash, Esc=quit"'
alias tedrej='cd ~/.ted/todos && fzf --bind "enter:execute-silent(mv {} ~/.ted/rejected/)+reload(find . -name \"*.md\" -type f)" --preview "cat {}" --header "Enter=reject, Esc=quit"'