# log_parse

Parse text log lines into JSON records using regex extraction. Each line is matched against a regex, and capture groups are mapped to field names.

## Usage

### Tokenize by whitespace

```bash
fimod s -i server.log -m @log_parse --arg regex="(\S+)"
```

### Map tokens to named fields

```bash
fimod s -i server.log -m @log_parse \
  --arg regex="(\S+) (\S+) \[(.+?)\] \"(.+?)\" (\d+) (\d+)" \
  --arg fields="ip,user,timestamp,request,status,bytes"
```

## Example

**Input** (`server.log`):
```
192.168.1.1 alice [2024-01-15] "GET /api" 200 1234
10.0.0.5 bob [2024-01-15] "POST /login" 401 56
```

**Command**:
```bash
fimod s -i server.log -m @log_parse \
  --arg regex="(\S+) (\S+) \[(.+?)\] \"(.+?)\" (\d+) (\d+)" \
  --arg fields=ip,user,timestamp,request,status,bytes
```

**Output**:
```json
[
  {"ip": "192.168.1.1", "user": "alice", "timestamp": "2024-01-15", "request": "GET /api", "status": "200", "bytes": "1234"},
  {"ip": "10.0.0.5", "user": "bob", "timestamp": "2024-01-15", "request": "POST /login", "status": "401", "bytes": "56"}
]
```

Without `--arg fields`, the output is a list of token arrays.

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `regex` | No | Regex with capture groups (default: `(\S+)` — split by whitespace) |
| `fields` | No | Comma-separated field names to map capture groups to |
