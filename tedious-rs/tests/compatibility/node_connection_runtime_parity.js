const tedious = require('/Users/neverson/myspace/orbitsql/tedious-node/lib/tedious.js');

const port = Number(process.argv[2]);
const mode = process.argv[3] || 'normal';
const retry = mode === 'retry';
const ntlm = mode === 'ntlm';
const fedauth = mode === 'fedauth';
const requestMode = mode === 'request';
const cancelMode = mode === 'cancel';
const closeMode = mode === 'close';
const timeoutMode = mode === 'timeout';
const events = [];
const authentication = ntlm
  ? { type: 'ntlm', options: { userName: 'user', password: 'password', domain: '' } }
  : fedauth
    ? { type: 'azure-active-directory-access-token', options: { token: 'fed-auth-token' } }
    : { type: 'default', options: { userName: 'sa', password: 'password' } };
const connection = new tedious.Connection({
  server: '127.0.0.1',
  authentication,
  options: {
    port,
    database: 'tedious_rs',
    encrypt: false,
    connectTimeout: 2000,
    requestTimeout: timeoutMode ? 200 : 2000,
    cancelTimeout: timeoutMode ? 100 : 0,
    maxRetriesOnTransientErrors: retry ? 2 : 3,
    connectionRetryInterval: 1,
    debug: { token: true }
  }
});

connection.on('connect', error => {
  events.push(error ? 'connectError' : 'connect');
  if (error || (!requestMode && !cancelMode && !closeMode && !timeoutMode)) {
    connection.close();
    return;
  }
  const request = new tedious.Request('SELECT 1', requestError => {
    events.push(requestError ? (cancelMode || closeMode || timeoutMode ? `requestCallbackError:${requestError.code}` : 'requestCallbackError') : 'requestCallback');
    if (cancelMode && requestError) {
      const secondRequest = new tedious.Request('SELECT 2', secondError => {
        events.push(secondError ? 'secondCallbackError' : 'secondCallback');
      });
      secondRequest.on('requestCompleted', () => {
        events.push('secondRequestCompleted');
        connection.close();
      });
      connection.execSqlBatch(secondRequest);
    }
  });
  request.on('requestCompleted', () => {
    if (!cancelMode && !closeMode && !timeoutMode) {
      events.push('requestCompleted');
      connection.close();
    }
  });
  request.on('done', () => events.push('done'));
  request.on('error', () => events.push('requestError'));
  if (cancelMode) connection.execSqlBatch(request);
  else if (closeMode) connection.execSqlBatch(request);
  else if (timeoutMode) connection.execSqlBatch(request);
  else connection.execSql(request);
  if (cancelMode) setTimeout(() => connection.cancel(), 30);
  if (closeMode) setTimeout(() => connection.close(), 30);
});
connection.on('retry', () => events.push('retry'));
connection.on('rerouting', () => events.push('rerouting'));
connection.on('debug', message => {
  if (message.startsWith('State change: ')) events.push(message);
});
connection.on('error', () => events.push('error'));
connection.on('end', () => {
  events.push('end');
  process.stdout.write(JSON.stringify(events));
});
connection.connect();
