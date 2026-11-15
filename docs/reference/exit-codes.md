# 🚦 Exit Codes

## ✅ Check mode (`--check`)

`--check` suppresses stdout and uses the **truthiness** of the transform result as the exit code.

| Exit code | Meaning |
|-----------|---------|
| **0** | ✅ Result is truthy |
| **1** | ❌ Result is falsy |

### Truthiness table

| Value | Truthy? |
|-------|---------|
| `null` | ❌ falsy |
| `false` | ❌ falsy |
| `0`, `0.0` | ❌ falsy |
| `""` | ❌ falsy |
| `[]`, `{}` | ❌ falsy |
| Everything else | ✅ truthy |

```bash
# ✅ Validate a record
fimod s -i record.json -e 'data.get("email") and data.get("name")' --check

# 🔀 In a shell conditional
if fimod s -i config.json -m validate.py --check; then
    echo "✅ Config is valid"
else
    echo "❌ Config has errors" >&2
    exit 1
fi
```

---

## 🚦 `set_exit(code)`

`set_exit(code)` sets the process exit code from inside a mold:

- `code` is an integer **0–255**
- The mold continues executing to completion after the call
- Returns `None`

```python
def transform(data, args, env, headers, **_):
    if not data.get("valid"):
        set_exit(1)
    return data
```

---

## 🔀 Interaction between `set_exit` and `--check`

!!! warning "set_exit takes priority"
    When both are active, `set_exit` overrides `--check` for the exit code. Stdout is still suppressed by `--check`.

```bash
# set_exit(2) inside validate.py overrides --check truthiness
fimod s -i record.json -m validate.py --check
echo $?   # could be 2, not 0 or 1
```

### Advanced: specific exit codes per validation failure

```python
# validate.py
def transform(data, args, env, headers, **_):
    if "host" not in data:
        set_exit(2)   # 🔴 missing required field
        return data
    if not data.get("port"):
        set_exit(3)   # 🟡 missing port
        return data
    return True       # ✅ --check sees truthy → exit 0
```

```bash
fimod s -i config.json -m validate.py --check
case $? in
    0) echo "✅ OK" ;;
    2) echo "❌ Missing host" >&2 ;;
    3) echo "⚠️  Missing port" >&2 ;;
esac
```

## 🔌 Closed stdout

If the reader of stdout closes the pipe early (for example `fimod s -i data.json -e 'data' | head -1`), fimod stops writing and ends quietly. This is not an error and does not change the exit status: it is `0`, or the code requested through `set_exit()`. Any other failure to write to stdout, such as a full disk, is an error with a message on stderr: the command exits with `1`, or keeps its own non-zero exit code (for example `1` for `fimod mold test` with a failing case) and still prints the message.

The rule applies to every command, not only `fimod shape`: `mold list`, `mold test`, `registry list`, `setup …` and `monty repl` end quietly too, and `fimod mold test` still exits `1` when a case fails. A write failure other than a closed pipe makes a command that would have succeeded fail with `Failed to write output to stdout`.

```bash
fimod s -i big.json -e 'data' | head -1
echo "${PIPESTATUS[0]}"   # 0
```
