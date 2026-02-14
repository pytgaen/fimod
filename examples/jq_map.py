def transform(data):
    """
    Map/Transform objects to a new structure.
    Equivalent to: jq 'map({user: .name, contact: .email})'
    """

    if not isinstance(data, list):
        return {"user": data.get("name"), "contact": data.get("email")}

    return [
        {
            "user": item.get("name"),
            "contact": item.get("email"),
            "is_active": item.get("status") == "active"
        }
        for item in data
    ]
