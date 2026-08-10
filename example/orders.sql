-- Orders of a period, by status
SELECT
    o.id,
    o.status,
    o.total
FROM orders o
WHERE o.created_at >= :'start_date'
  AND o.created_at < :'end_date'
  AND o.status = :'status'
ORDER BY o.created_at;
