SELECT
    path,
    status,
    additions,
    deletions
FROM changed_files
WHERE pull_request_id = :pull_request_id
  AND is_generated = FALSE
ORDER BY additions + deletions DESC;
