const IncomingMessageStream = require('/Users/neverson/myspace/orbitsql/tedious-node/lib/incoming-message-stream.js');
const Debug = require('/Users/neverson/myspace/orbitsql/tedious-node/lib/debug.js');

function packet(type, last, data) {
  const buffer = Buffer.alloc(8 + data.length);
  buffer.writeUInt8(type, 0);
  buffer.writeUInt8(last ? 1 : 0, 1);
  buffer.writeUInt16BE(buffer.length, 2);
  buffer.writeUInt16BE(0, 4);
  buffer.writeUInt8(1, 6);
  buffer.writeUInt8(0, 7);
  data.copy(buffer, 8);
  return buffer;
}

const events = [];
const incoming = new IncomingMessageStream(new Debug());
let ended = false;

incoming.on('data', message => {
  events.push(`message:${message.type}`);
  message.on('data', chunk => events.push(`data:${chunk.toString('hex')}`));
  message.on('end', () => {
    ended = true;
    events.push('messageEnd');
    if (events.includes('secondWrite:false')) {
      process.stdout.write(JSON.stringify(events));
    }
  });
});
incoming.on('error', error => {
  events.push(`error:${error.message}`);
  process.stdout.write(JSON.stringify(events));
  process.exit(1);
});
incoming.on('end', () => process.stdout.write(JSON.stringify(events)));

const first = packet(0x04, false, Buffer.from('first'));
const second = packet(0x04, true, Buffer.from('second'));
incoming.write(first, () => {
  events.push(`firstWrite:${ended}`);
  incoming.pause();
  incoming.write(second, () => {
    events.push(`secondWrite:${ended}`);
    incoming.end();
  });
  incoming.resume();
});
