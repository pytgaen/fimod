# Configuration
Output video/demo-chaining.gif

Set FontFamily "JetBrainsMono Nerd Font"
Set FontSize 18
Set Width 1200
Set Height 500
Set Padding 40
Set Theme "Catppuccin Mocha"

# Setup silent
Hide
Type "cat << 'EOF' > data.json" Enter
Type '[{"name":"Zoe", "id": 10}, {"name":"Alex", "id": 40}, {"name":"Marc", "id": 20}]' Enter
Type "EOF" Enter
Type "clear" Enter
Show

# View the input
Type "# View the data.json file" Enter
Type "cat data.json" Enter
Sleep 0.7s
Type "# Names are in mixed case and IDs are unsorted, let's fix this with it_sort_by" Enter
Sleep 1.5s
Enter

# Demo
Type "# Fix using chain expressions and built-in helpers without importing"
Sleep 0.5s
Enter

Type "fimod s -i data.json -e 'it_sort_by(data, "
Type '"id")'
Type "' -e '[{"
Type '"id": u["id"], "user": u["name"].upper()}'
Type " for u in data]'"
Sleep 0.5s
Enter
Sleep 3.2s

# Teardown
Hide
Type "rm data.json" Enter
