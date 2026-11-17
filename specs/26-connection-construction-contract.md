# Contrato de Connection

O estado mapeia fedAuthRequired, inTransaction, transaction depth, retry count, routing data, MessageIO e database collation.

Os cancel/request timers usam connect 15s, request 15s, cancel 5s e retry interval 500ms.

Métodos cobertos: connect, close, execSql, execSqlBatch, newBulkLoad, execBulkLoad, prepare, unprepare, execute, callProcedure, beginTransaction, commitTransaction, rollbackTransaction, saveTransaction, transaction, cancel, reset, initialiseConnection, performReRouting, performTransientFailureRetry e performSentLogin7WithNTLMLogin.

Erros cobertos: prelogin inválido, login falho, cancel sem ACK, request concorrente, retry transitório e rerouting.
