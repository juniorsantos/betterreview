def visible_files(files: list[str], query: str) -> list[str]:
    normalized_query = query.strip().lower()
    if not normalized_query:
        return files

    return [path for path in files if normalized_query in path.lower()]
