const crypto = require('crypto');
const path = require('path');

const root = path.resolve(__dirname, '../../../tedious-node/lib');
const hex = value => Buffer.from(value).toString('hex');
const load = name => require(path.join(root, name));

const name = process.argv[2];
let value;

if (name === 'login7') {
  const Login7Payload = load('login7-payload');
  const payload = new Login7Payload({
    tdsVersion: 0x72090002,
    packetSize: 4096,
    clientProgVer: 1,
    clientPid: 7,
    connectionId: 0,
    clientTimeZone: 0,
    clientLcid: 0x409
  });
  payload.hostname = 'compat';
  payload.userName = 'sa';
  payload.password = 'x';
  payload.appName = 'tedious-rs';
  payload.serverName = 'localhost';
  payload.database = 'tedious_rs';
  value = payload.toBuffer();
} else if (name === 'sql_batch') {
  const SqlBatchPayload = load('sqlbatch-payload');
  const payload = new SqlBatchPayload('SELECT 1', Buffer.alloc(8), { tdsVersion: '7_4' });
  value = Buffer.concat([...payload]);
} else if (name === 'rpc') {
  const RpcRequestPayload = load('rpcrequest-payload');
  const Int = load('data-types/int');
  const payload = new RpcRequestPayload(
    'sp_executesql',
    [{ name: 'p', output: false, value: 7, type: Int }],
    Buffer.alloc(8),
    { tdsVersion: '7_4' }
  );
  value = Buffer.concat([...payload]);
} else if (name === 'tvp') {
  const TVP = load('data-types/tvp');
  const Int = load('data-types/int');
  const parameter = {
    value: {
      name: 'CompatType',
      schema: 'dbo',
      columns: [{ name: 'id', type: Int }],
      rows: [[7]]
    }
  };
  value = Buffer.concat([
    TVP.generateTypeInfo(parameter),
    TVP.generateParameterLength(parameter),
    ...TVP.generateParameterData(parameter)
  ]);
} else if (name === 'int_parameter') {
  const Int = load('data-types/int');
  const parameter = { value: 7 };
  value = Buffer.concat([Int.generateTypeInfo(), Int.generateParameterLength(parameter), ...Int.generateParameterData(parameter)]);
} else if (name === 'bulk') {
  const BulkLoad = load('bulk-load');
  const Int = load('data-types/int');
  const bulk = new BulkLoad('dbo.compat', null, { tdsVersion: '7_4', packetSize: 4096 }, {}, () => {});
  bulk.addColumn('id', Int, { nullable: false });
  const chunks = [];
  value = new Promise((resolve, reject) => {
    bulk.rowToPacketTransform.on('data', chunk => chunks.push(chunk));
    bulk.rowToPacketTransform.on('error', reject);
    bulk.rowToPacketTransform.on('end', () => resolve(Buffer.concat(chunks)));
    bulk.rowToPacketTransform.end({ id: 7 });
  });
} else {
  process.stderr.write(`unsupported compatibility case: ${name}`);
  process.exit(2);
}

Promise.resolve(value).then(result => process.stdout.write(hex(result))).catch(error => {
  process.stderr.write(error.message);
  process.exit(1);
});
