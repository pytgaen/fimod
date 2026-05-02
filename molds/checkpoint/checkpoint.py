"""
Fail with exit 1 if data is empty at this point in the pipeline.

"Empty" means: None, empty string, empty list, or empty dict.
Data is passed through unchanged so downstream steps still receive it.

Usage:
  fimod s -i response.json -m transform.py -m @checkpoint -m publish.py
  fimod s -i response.json -m transform.py -m @checkpoint --arg label='after extract' -m publish.py
"""
# fimod: arg=label     "Description shown in the error message (default: checkpoint)"
# fimod: arg=exit_code "Exit code on failure (default: 1)"


def transform(data, args, pipeline, **_):
    step = pipeline.current_step()
    label = args.get("label", "checkpoint")
    exit_code = int(args.get("exit_code", "1"))

    empty = (
        data is None
        or data == ""
        or (isinstance(data, (list, dict)) and len(data) == 0)
    )

    if empty:
        msg_error(
            f"[step {step.get('index') + 1}/{pipeline.length()}] {label}: data is empty"
        )
        step.set('exit', exit_code)

    return data
