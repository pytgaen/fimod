# Configuration
Output video/demo-slurp.gif
Set FontSize 18
Set Width 1200
Set Height 600
Set Padding 40
Set Theme "Catppuccin Mocha"

# Setup silent
Hide
Type "printf 'server: nginx\nport: 80\n' > base.yaml" Enter
Type "printf 'port = 443\nssl = true\n' > prod.toml" Enter
Type "clear" Enter
Show

# View the input
Type "# View the configuration files" Enter
Type "cat base.yaml" Enter
Sleep 0.7s
Type "cat prod.toml" Enter
Type "# We have a YAML base config and TOML overrides, let's merge them" Enter
Sleep 1.5s
Enter

# Demo
Type "# Merge a base YAML config with TOML prod overrides"
Sleep 0.5s
Enter

Type "fimod s -i base.yaml -i prod.toml -s -e '"
Enter
Type "def transform(data, args, env, headers):"
Enter
Type "    data[0].update(data[1])"
Enter
Type "    return data[0]"
Enter
Type "' --output-format json"
Sleep 0.5s
Enter
Sleep 3.2s

# Teardown
Hide
Type "rm base.yaml prod.toml" Enter
