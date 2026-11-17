const path = require('path');

const Request = require(path.resolve(__dirname, '../../../tedious-node/lib/request'));
const mode = process.argv[2];
const observations = [];
const request = new Request('SELECT 1', error => {
  observations.push(error ? `callback:error:${error.code || ''}` : 'callback:ok');
});
request.on('error', error => observations.push(`event:error:${error.code || ''}`));
request.on('requestCompleted', () => observations.push('event:requestCompleted'));

if (mode === 'success') {
  request.callback(null);
} else if (mode === 'failure') {
  const error = new Error('sanitized failure');
  error.code = 'ERUNTIME';
  request.callback(error);
} else if (mode === 'preparing_failure') {
  request.preparing = true;
  const error = new Error('sanitized prepare failure');
  error.code = 'EPREPARE';
  request.callback(error);
} else {
  process.exit(2);
}

process.stdout.write(JSON.stringify(observations));
