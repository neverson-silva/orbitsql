# Message stream incremental gap

`tedious-node/src/incoming-message-stream.ts` publica cada `Message` no `push` antes de continuar o processamento dos bytes restantes. Se o lote contém uma mensagem completa seguida de um pacote cujo comprimento é menor que `HEADER_LENGTH`, a mensagem completa já foi observada quando o callback recebe `ConnectionError`.

O `Result<Vec<Message>, MessageStreamError>` legado não consegue transportar simultaneamente os valores produzidos e o erro sem quebrar consumidores que constroem `MessageStreamError` como tupla. A adaptação mínima mantém o `Result` e o estado terminal, preservando as mensagens completas em `take_pending_messages`. Eventos já produzidos de uma mensagem parcial ficam disponíveis em `take_pending_events`; nenhuma mensagem incompleta é promovida a mensagem válida.

Os métodos são aditivos e só precisam ser consultados depois de um `feed` ou `resume` que retornou erro. O erro original e as tentativas posteriores continuam com a mesma categoria, mensagem e estado terminal.

A fachada pública `tedious_rs::incoming_message_stream::IncomingMessageStream` delega esses dois métodos sem alterar `feed`, `feed_incremental` ou `resume`. Os testes externos `public_stream_recovers_complete_message_after_error` e `public_stream_recovers_partial_events_after_error` verificam a recuperação por essa fachada.
