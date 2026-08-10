-- User counts, total and last 30 days
SELECT
    count(*) AS users,
    count(*) FILTER (WHERE created_at >= now() - interval '30 days') AS recent
FROM users;
