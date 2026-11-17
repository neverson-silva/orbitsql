const path = require('path');

const root = path.resolve(__dirname, '../../../tedious-node/lib');
const PreloginPayload = require(path.join(root, 'prelogin-payload.js'));
const Login7Payload = require(path.join(root, 'login7-payload.js'));

const prelogin = new PreloginPayload({
  encrypt: true,
  version: { major: 1, minor: 2, build: 3, subbuild: 4 }
});
const preloginData = Buffer.from(prelogin.data);
let cursor = 0;
while (preloginData[cursor] !== 0xff) {
  const token = preloginData[cursor];
  const offset = preloginData.readUInt16BE(cursor + 1);
  const length = preloginData.readUInt16BE(cursor + 3);
  if (token === 0x05) {
    preloginData.fill(0, offset, offset + length);
  }
  cursor += 5;
}

const login7 = new Login7Payload({
  tdsVersion: 0x74000004,
  packetSize: 4096,
  clientProgVer: 0,
  clientPid: 1234,
  connectionId: 0,
  clientTimeZone: 0,
  clientLcid: 0x409
});
login7.userName = 'sa';
login7.password = 'password';
login7.serverName = 'localhost';
login7.appName = 'Tedious';
login7.hostname = 'host';
login7.libraryName = 'Tedious';
login7.database = 'tedious_rs';
login7.clientId = Buffer.from([1, 2, 3, 4, 5, 6]);

const login7Ntlm = new Login7Payload({
  tdsVersion: 0x74000004,
  packetSize: 4096,
  clientProgVer: 0,
  clientPid: 1234,
  connectionId: 0,
  clientTimeZone: 0,
  clientLcid: 0x409
});
login7Ntlm.serverName = 'localhost';
login7Ntlm.sspi = Buffer.from([1, 2, 3, 4]);

const login7FedAuth = new Login7Payload({
  tdsVersion: 0x74000004,
  packetSize: 4096,
  clientProgVer: 0,
  clientPid: 1234,
  connectionId: 0,
  clientTimeZone: 0,
  clientLcid: 0x409
});
login7FedAuth.serverName = 'localhost';
login7FedAuth.fedAuth = { type: 'ADAL', echo: true, workflow: 'integrated' };

process.stdout.write(JSON.stringify({
  prelogin: preloginData.toString('hex'),
  login7: login7.toBuffer().toString('hex'),
  login7Ntlm: login7Ntlm.toBuffer().toString('hex'),
  login7FedAuth: login7FedAuth.toBuffer().toString('hex')
}));
