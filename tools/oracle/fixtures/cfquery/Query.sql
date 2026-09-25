SELECT u.id, u.name
FROM users u
WHERE u.active = #isActive()#
  AND u.manager_id = #getCurrentUser().getTeam().getManagerId()#
  AND u.created_at > #variables.cutoff#
ORDER BY u.name
