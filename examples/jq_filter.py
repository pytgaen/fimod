def transform(data):
    """
    Filter a list of objects.
    Equivalent to: jq 'map(select(.age > 30 and .role == "admin"))'
    """

    # Ensure input is a list
    if not isinstance(data, list):
        return data

    # Return filtered list
    return [
        item for item in data
        if item.get("age", 0) > 30 and item.get("role") == "admin"
    ]
