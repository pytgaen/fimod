# Configuration
Output video/demo-http.gif
Set FontSize 18
Set Width 1200
Set Height 500
Set Padding 40
Set Theme "Catppuccin Mocha"

# Setup silent
Hide
Type "clear" Enter
Show

# View the input
Type "# Let's check the GitHub API response for ty" Enter
Type "curl -s https://api.github.com/repos/astral-sh/ty | grep star" Enter
Sleep 1.5s
Type "# We want to extract just the repository name and stargazers_count" Enter
Sleep 0.5s
Enter

# Demo
Type "# Replace curl | jq: direct queries to the GitHub API" Enter
Sleep 0.5s

Type "fimod s -i https://api.github.com/repos/astral-sh/ty -e 'data["
Type '"name"] + ": " + str(data["stargazers_count"]) + " stars"'
Type "' --output-format txt"
Sleep 0.5s
Enter
Sleep 3.2s
