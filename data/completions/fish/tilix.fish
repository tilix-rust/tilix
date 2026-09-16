# Fish completion for tilix

complete -c tilix -s h -l help -d "Show help options"
complete -c tilix -s v -l version -d "Show version information"
complete -c tilix -s p -l preferences -d "Open Preferences dialog"
complete -c tilix -l quake -d "Launch or present Quake window"
complete -c tilix -l quake-toggle -d "Toggle Quake window visibility"

complete -c tilix-rust -w tilix
