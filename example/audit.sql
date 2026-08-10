-- Audit trail of a table, ignoring quoted lookalikes
/* :'not_a_variable' inside a block comment
   /* and inside a nested one */ is ignored too */
SELECT
    a.id,
    a.action,
    'literal :''also_not_a_variable''' AS note,
    $doc$ :'nor_this_one' $doc$ AS doc
FROM audit a
WHERE a."column :'still_not_one'" IS NOT NULL  -- :'never_this'
  AND a.table_name = :'table_name'
  AND a.action = :'action'
ORDER BY a.id DESC
LIMIT :'row_limit';
