"""
Transform JSON/YAML config to .env format.

Usage:
  fimod s -i config.yaml -m @env_to_dotenv -o .env
"""
# fimod: output-format=txt

def transform(data, args, env, headers):
    if not isinstance(data, dict):
        return data

    return "\n".join(f"{k}={v}" for k, v in data.items()) + "\n"
