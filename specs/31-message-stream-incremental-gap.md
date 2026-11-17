# Message stream incremental

`feed_incremental` e `resume_incremental` preservam eventos públicos `MessageStreamEvent::MessageStarted`, `Data` e `MessageEnded`.

`feed_incremental_with_sink` e `resume_incremental_with_sink` transportam `MessageStreamControl::Pending` sem perder o primeiro pacote nem payload vazio.
