-- Details of one user
SELECT
    id,
    email,
    created_at
FROM users
WHERE id = :'user_id';
