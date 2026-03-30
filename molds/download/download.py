"""
Download a file from a URL (wget-like).

Usage:
  fimod s -i https://example.com/file.tar.gz -m @download
  fimod s -i https://example.com/file.tar.gz -m @download --arg out=myfile.tar.gz
"""
# fimod: input-format=http
# fimod: arg=out  Output filename (defaults to the last path segment of the URL)

def transform(data, args, **_):
    url = args.get("url", "") or data.get("url", "")
    filename = args.get("out") or url.split("?")[0].rstrip("/").split("/")[-1]
    set_output_format("raw")
    set_output_file(filename)
    return data["body"]
