"""
Parse log lines into structured records via regex capture groups.

Usage:
  fimod s -i server.log -m @log_parse --arg regex="(\S+)"
  fimod s -i server.log -m @log_parse \
    --arg regex="(\S+) (\S+) \[(.+?)\] \"(.+?)\" (\d+) (\d+)" \
    --arg fields="ip,user,timestamp,request,status,bytes"
"""
# fimod: input-format=lines
# fimod: output-format=json

def transform(data, args, env, headers):
    try:
        regex_arg = args.get("regex", "")
        fields_arg = args.get("fields", "")
    except NameError:
        regex_arg = ""
        fields_arg = ""

    # Use the original default regex if regex_arg is empty
    regex = regex_arg if regex_arg else r"(\S+)"

    fields = [f.strip() for f in fields_arg.split(",")] if fields_arg else []

    results = []
    for line in data:
        if not line.strip():
            continue

        matches = re_findall(regex, line)
        # re_findall with N groups returns [[g1,g2,...], ...] per match.
        # For log parsing we typically have one match per line, so flatten.
        if matches and isinstance(matches[0], list):
            matches = matches[0]
        if fields:
            record = {}
            for i in range(min(len(fields), len(matches))):
                record[fields[i]] = matches[i]
            results.append(record)
        else:
            results.append(matches)

    return results
