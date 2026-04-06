# Configuration
Output video/demo-hero.gif
Set FontSize 18
Set Width 1200
Set Height 600
Set Padding 40
Set Theme "Catppuccin Mocha"

# Setup silent
Hide
Type "cat << 'EOF' > users.json" Enter
Type '[{"name": "Alice", "age": 28, "active": true}, {"name": "Bob", "age": 42, "active": false}, {"name": "Charlie", "age": 35, "active": true}]' Enter
Type "EOF" Enter
Type "clear" Enter
Show

# View the input
Type "# View the users.json file" Enter
Type "cat users.json" Enter
Sleep 0.7s
Type "# We have active and inactive users, we want only active ones as CSV" Enter
Sleep 1.5s
Enter

# Demo
Type "# Filter active users and export to CSV. Simple, it's just Python!"
Sleep 0.5s
Enter

Type "fimod s -i users.json -e '[u for u in data if u["
Type '"active"'
Type "]]' --output-format csv"
Sleep 0.5s
Enter
Sleep 3.2s

# Teardown
Hide
Type "rm users.json" Enter
