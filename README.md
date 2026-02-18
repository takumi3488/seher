# Seher


Seher is a CLI tool that waits for Claude's Rate Limit to reset, then executes a specified prompt using Claude Code.


## How it works


By default, it retrieves Chrome's cookie information and uses it to call Claude's API to get the Rate Limit reset time. The browser and profile from which cookies are retrieved can be changed with options.


## Supported Browsers

Seher supports reading cookies from the following browsers:

### Chromium-based browsers
- Chrome
- Microsoft Edge
- Brave
- Chromium
- Vivaldi
- Comet (Perplexity AI browser)
- Dia (The Browser Company)
- ChatGPT Atlas (OpenAI browser)

### Other browsers
- Firefox (all platforms)
- Safari (macOS only - uses sandboxed cookies location)

All Chromium-based browsers use the same cookie storage format and encryption. Firefox uses a different SQLite schema without encryption. Safari uses a proprietary binary format on macOS.

**Note:** On recent versions of macOS, Safari cookies are stored in a sandboxed location: `~/Library/Containers/com.apple.Safari/Data/Library/Cookies/Cookies.binarycookies`


## Installation


```sh
cargo install sehercode # Note that it's not seher!
```


## Usage


```sh
# Default
seher "fix bugs"
# Launch vim to input a prompt
seher
# Change the browser and profile from which cookies are retrieved
seher --browser edge --profile "Profile 1" "fix bugs"
# Use Firefox
seher --browser firefox --profile "default-release" "fix bugs"
# Use Safari (macOS only)
seher --browser safari "fix bugs"
# Most Claude Code options can be used as is
seher --chrome --disallowedTools "Bash(git:*)" --permission-mode bypassPermissions "fix bugs"
```


It is recommended to alias frequently used options as follows:


```sh
alias shr="seher --profile 'Profile 1' --permission-mode bypassPermissions"
```

