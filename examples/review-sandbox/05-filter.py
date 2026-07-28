def visible_files(
    files: list[str],
    query: str,
    include_generated: bool = False,
) -> list[str]:
    normalized_query = query.strip().lower()

    return [
        path
        for path in files
        if (include_generated or "/generated/" not in path)
        and normalized_query in path.lower()
    ]
