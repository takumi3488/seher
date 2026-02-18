# Seher


Seher is a CLI tool that waits for Claude's Rate Limit to reset, then executes a specified prompt using Claude Code.


## How it works


By default, it retrieves Chrome's cookie information and uses it to call Claude's API to get the Rate Limit reset time. The browser and profile from which cookies are retrieved can be changed with options.


## Installation


```sh
cargo install sehercode # Note that it's not seher!
```


## Usage


```sh
# Default (runs in plan mode)
seher "fix bugs"
# Launch vim to input a prompt
seher
# Change the browser and profile from which cookies are retrieved
seher --browser edge --profile "Profile 1" "fix bugs"
# Most Claude Code options can be used as is
seher --chrome --disallowedTools "Bash(git:*)" --permission-mode bypassPermissions "fix bugs"
```


It is recommended to alias frequently used options as follows:


```sh
alias shr="seher --profile 'Profile 1' --permission-mode bypassPermissions"
```

