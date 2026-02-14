# env_to_dotenv

Transform a JSON or YAML configuration file into `.env` format, ready to be sourced by bash or Docker Compose.

## Usage

```bash
fimod s -i config.yaml -m @env_to_dotenv -o .env
```

## Example

**Input** (`config.yaml`):
```yaml
DB_HOST: localhost
DB_PORT: 5432
SECRET_KEY: s3cret
```

**Output** (`.env`):
```
DB_HOST=localhost
DB_PORT=5432
SECRET_KEY=s3cret
```

### From JSON

```bash
fimod s -i config.json -m @env_to_dotenv -o .env
```

### Pipe to stdout

```bash
fimod s -i config.yaml -m @env_to_dotenv
```
