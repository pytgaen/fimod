# yaml_merge

Merge path-value pairs into a YAML configuration. Ideal for patching Kubernetes manifests, Docker Compose files, or any YAML config in CI pipelines.

## Usage

```bash
fimod s -i deployment.yaml -m @yaml_merge --arg set="spec.replicas=3,metadata.labels.env=prod"
```

## Example

**Input** (`deployment.yaml`):
```yaml
metadata:
  name: myapp
  labels:
    app: myapp
spec:
  replicas: 1
```

**Output**:
```yaml
metadata:
  name: myapp
  labels:
    app: myapp
    env: prod
spec:
  replicas: 3
```

### In-place editing

```bash
fimod s -i deployment.yaml -m @yaml_merge --arg set="spec.replicas=5" --in-place
```

### Type coercion

Values are automatically parsed: `true`/`false` become booleans, integers stay integers, everything else is a string.

```bash
fimod s -i config.yaml -m @yaml_merge --arg set="debug=true,server.port=8080,server.host=0.0.0.0"
```

## Args

| Arg | Required | Description |
|-----|----------|-------------|
| `set` | Yes | Comma-separated `dotpath=value` assignments |
