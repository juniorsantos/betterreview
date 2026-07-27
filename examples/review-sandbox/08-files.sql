SELECT
    path,
    additions,
    deletions
FROM changed_files
WHERE pull_request_id = :pull_request_id
ORDER BY path ASC;
