import { availableParallelism } from "node:os";
import { performance } from "node:perf_hooks";
import { isMainThread, parentPort, Worker } from "node:worker_threads";

const objectCount = Number(process.env.ORBITSQL_BENCH_OBJECTS ?? 500000);
const rounds = Number(process.env.ORBITSQL_BENCH_ROUNDS ?? 24);
const samples = Number(process.env.ORBITSQL_BENCH_SAMPLES ?? 15);
const topK = 50;

function compute(values, start, end, workRounds) {
  let checksum = 0;
  for (let index = start; index < end; index += 1) {
    let value = values[index] ?? index;
    for (let round = 0; round < workRounds; round += 1) {
      value = Math.imul(value ^ (value >>> 13), 0x45d9f3b);
      value ^= value >>> 16;
    }
    checksum ^= value;
  }
  return { checksum, candidateCount: end - start, topK };
}

function percentile(values, fraction) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[
    Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)
  ];
}

function stats(values) {
  return {
    p50: percentile(values, 0.5),
    p95: percentile(values, 0.95),
    p99: percentile(values, 0.99),
    max: Math.max(...values),
  };
}

function immediate() {
  return new Promise((resolve) => setImmediate(resolve));
}

function workerMemory(worker) {
  return new Promise((resolve, reject) => {
    const id = `memory:${Math.random()}`;
    const errorListener = (error) => {
      worker.off("message", listener);
      reject(error);
    };
    const listener = (message) => {
      if (message.id !== id) return;
      worker.off("message", listener);
      worker.off("error", errorListener);
      resolve(message.heapUsedMb);
    };
    worker.on("message", listener);
    worker.on("error", errorListener);
    worker.postMessage({ type: "memory", id });
  });
}

function request(worker, payload) {
  return new Promise((resolve, reject) => {
    const errorListener = (error) => {
      worker.off("message", listener);
      reject(error);
    };
    const listener = (message) => {
      if (message.id !== payload.id) return;
      worker.off("message", listener);
      worker.off("error", errorListener);
      resolve(message);
    };
    worker.on("message", listener);
    worker.on("error", errorListener);
    worker.postMessage(payload);
  });
}

async function createWorker(mode, values) {
  const worker = new Worker(new URL(import.meta.url));
  const id = `init:${Math.random()}`;
  const started = performance.now();
  const payload =
    mode === "coordinator"
      ? {
          type: "init-coordinator",
          id,
          buffer: values.buffer,
          count: values.length,
          workers: 4,
        }
      : mode === "shared"
        ? {
            type: "init-shared",
            id,
            buffer: values.buffer,
            count: values.length,
          }
        : { type: "init-clone", id, values: Array.from(values) };
  await request(worker, payload);
  return {
    worker,
    initMs: performance.now() - started,
    heapUsedMb: await workerMemory(worker),
    coordinator: mode === "coordinator",
  };
}

async function closeWorkers(entries) {
  await Promise.all(
    entries.map(async (entry) => {
      if (entry.coordinator)
        await request(entry.worker, {
          type: "shutdown",
          id: `shutdown:${Math.random()}`,
        });
      await entry.worker.terminate();
    }),
  );
}

async function measureSamples(fn, count = samples) {
  for (let index = 0; index < 4; index += 1) await fn();
  const values = [];
  for (let index = 0; index < count; index += 1) {
    const started = performance.now();
    await fn();
    values.push(performance.now() - started);
  }
  return values;
}

async function measureEventLoop(fn, count = 8) {
  const gaps = [];
  let last = performance.now();
  const timer = setInterval(() => {
    const now = performance.now();
    gaps.push(now - last);
    last = now;
  }, 1);
  await immediate();
  for (let index = 0; index < count; index += 1) {
    await fn();
    await immediate();
  }
  clearInterval(timer);
  return Math.max(0, ...gaps);
}

async function measureWorkerRequests(
  entries,
  values,
  workRounds,
  count = samples,
) {
  let requestNumber = 0;
  return measureSamples(async () => {
    const id = `work:${requestNumber++}`;
    const started = performance.now();
    const result = await request(entries[0].worker, {
      type: "work",
      id,
      start: 0,
      end: values.length,
      rounds: workRounds,
    });
    return {
      elapsed: performance.now() - started,
      queueWait: result.startedAt - started,
    };
  }, count);
}

async function measureParallelRequests(
  entries,
  values,
  workRounds,
  count = samples,
) {
  let requestNumber = 0;
  return measureSamples(async () => {
    const id = `parallel:${requestNumber++}`;
    const started = performance.now();
    const width = Math.ceil(values.length / entries.length);
    const results = await Promise.all(
      entries.map((entry, index) =>
        request(entry.worker, {
          type: "work",
          id: `${id}:${index}`,
          start: index * width,
          end: Math.min(values.length, (index + 1) * width),
          rounds: workRounds,
        }),
      ),
    );
    return { elapsed: performance.now() - started, results };
  }, count);
}

async function saturationComparison(values) {
  const sharedWorkers = [];
  for (let index = 0; index < 4; index += 1)
    sharedWorkers.push(await createWorker("shared", values));
  const isolatedInteractive = await createWorker("shared", values);
  const backgroundWorkers = [];
  for (let index = 0; index < 3; index += 1)
    backgroundWorkers.push(await createWorker("shared", values));
  const sharedBackgroundMessages = sharedWorkers.map((entry, index) =>
    request(entry.worker, {
      type: "work",
      id: `shared-background:${index}`,
      start: 0,
      end: values.length,
      rounds: rounds * 8,
    }),
  );
  const backgroundMessages = backgroundWorkers.map((entry, index) =>
    request(entry.worker, {
      type: "work",
      id: `dedicated-background:${index}`,
      start: 0,
      end: values.length,
      rounds: rounds * 8,
    }),
  );
  await new Promise((resolve) => setTimeout(resolve, 2));
  const sharedStarted = performance.now();
  const sharedResult = await request(sharedWorkers[0].worker, {
    type: "work",
    id: "saturated-shared",
    start: 0,
    end: values.length,
    rounds,
  });
  const isolatedStarted = performance.now();
  const isolatedResult = await request(isolatedInteractive.worker, {
    type: "work",
    id: "saturated-isolated",
    start: 0,
    end: values.length,
    rounds,
  });
  await Promise.all([...sharedBackgroundMessages, ...backgroundMessages]);
  const output = {
    sharedWorkerQueueWaitMs: sharedResult.startedAt - sharedStarted,
    sharedWorkerTotalMs: performance.now() - sharedStarted,
    dedicatedInteractiveQueueWaitMs: isolatedResult.startedAt - isolatedStarted,
    dedicatedInteractiveTotalMs: performance.now() - isolatedStarted,
  };
  await closeWorkers([
    ...sharedWorkers,
    isolatedInteractive,
    ...backgroundWorkers,
  ]);
  return output;
}

async function main() {
  const values = new Uint32Array(
    new SharedArrayBuffer(objectCount * Uint32Array.BYTES_PER_ELEMENT),
  );
  for (let index = 0; index < values.length; index += 1)
    values[index] = Math.imul(index + 1, 2654435761);
  const direct = await measureSamples(() =>
    compute(values, 0, values.length, rounds),
  );
  const directBlock = await measureEventLoop(() =>
    compute(values, 0, values.length, rounds),
  );
  const oneShared = [await createWorker("shared", values)];
  const oneClone = [await createWorker("clone", values)];
  const clone2 = await Promise.all(
    Array.from({ length: 2 }, () => createWorker("clone", values)),
  );
  const clone4 = await Promise.all(
    Array.from({ length: 4 }, () => createWorker("clone", values)),
  );
  const clone8 = await Promise.all(
    Array.from({ length: 8 }, () => createWorker("clone", values)),
  );
  const pool2 = await Promise.all(
    Array.from({ length: 2 }, () => createWorker("shared", values)),
  );
  const pool4 = await Promise.all(
    Array.from({ length: 4 }, () => createWorker("shared", values)),
  );
  const pool8 = await Promise.all(
    Array.from({ length: 8 }, () => createWorker("shared", values)),
  );
  const coordinator = [await createWorker("coordinator", values)];
  const oneSharedSamples = await measureWorkerRequests(
    oneShared,
    values,
    rounds,
  );
  const oneCloneSamples = await measureWorkerRequests(oneClone, values, rounds);
  const pool2Samples = await measureParallelRequests(pool2, values, rounds);
  const pool4Samples = await measureParallelRequests(pool4, values, rounds);
  const pool8Samples = await measureParallelRequests(pool8, values, rounds);
  const coordinatorSamples = await measureWorkerRequests(
    coordinator,
    values,
    rounds,
  );
  const oneSharedBlock = await measureEventLoop(() =>
    request(oneShared[0].worker, {
      type: "work",
      id: `block-shared:${Math.random()}`,
      start: 0,
      end: values.length,
      rounds,
    }),
  );
  const pool4Block = await measureEventLoop(() =>
    measureParallelRequests(pool4, values, rounds, 1),
  );
  const saturation = await saturationComparison(values);
  const output = {
    environment: {
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      logicalCpu: availableParallelism(),
      objectCount,
      rounds,
      samples,
    },
    methodology: {
      workload:
        "CPU-bound candidate scoring over a compact catalog; synthetic kernel, not complete()",
      catalogSharing:
        "SharedArrayBuffer for shared mode; structured clone for clone mode",
      ranking: "each shard returns bounded top-K metadata",
      latency: "postMessage round trip plus worker execution",
    },
    baseline: {
      mainThread: stats(direct),
      mainThreadMaxEventLoopGapMs: directBlock,
    },
    worker: {
      singleShared: {
        latency: stats(oneSharedSamples),
        maxEventLoopGapMs: oneSharedBlock,
        initMs: oneShared[0].initMs,
        workerHeapMb: oneShared[0].heapUsedMb,
      },
      singleStructuredClone: {
        latency: stats(oneCloneSamples),
        initMs: oneClone[0].initMs,
        workerHeapMb: oneClone[0].heapUsedMb,
      },
      clone2: {
        initMs: clone2.reduce((sum, item) => sum + item.initMs, 0),
        workerHeapMb: clone2.reduce((sum, item) => sum + item.heapUsedMb, 0),
      },
      clone4: {
        initMs: clone4.reduce((sum, item) => sum + item.initMs, 0),
        workerHeapMb: clone4.reduce((sum, item) => sum + item.heapUsedMb, 0),
      },
      clone8: {
        initMs: clone8.reduce((sum, item) => sum + item.initMs, 0),
        workerHeapMb: clone8.reduce((sum, item) => sum + item.heapUsedMb, 0),
      },
      pool2: {
        latency: stats(pool2Samples),
        maxEventLoopGapMs: pool4Block,
        initMs: pool2.reduce((sum, item) => sum + item.initMs, 0),
        workerHeapMb: pool2.reduce((sum, item) => sum + item.heapUsedMb, 0),
      },
      pool4: {
        latency: stats(pool4Samples),
        initMs: pool4.reduce((sum, item) => sum + item.initMs, 0),
        workerHeapMb: pool4.reduce((sum, item) => sum + item.heapUsedMb, 0),
      },
      pool8: {
        latency: stats(pool8Samples),
        initMs: pool8.reduce((sum, item) => sum + item.initMs, 0),
        workerHeapMb: pool8.reduce((sum, item) => sum + item.heapUsedMb, 0),
      },
      coordinatorPool4: {
        latency: stats(coordinatorSamples),
        initMs: coordinator[0].initMs,
        workerHeapMb: coordinator[0].heapUsedMb,
      },
    },
    saturation,
    memory: {
      mainHeapUsedMb: process.memoryUsage().heapUsed / 1024 / 1024,
      sharedCatalogBytes: values.byteLength,
      structuredClonePerWorker: values.byteLength,
    },
  };
  await closeWorkers([
    ...oneShared,
    ...oneClone,
    ...clone2,
    ...clone4,
    ...clone8,
    ...pool2,
    ...pool4,
    ...pool8,
    ...coordinator,
  ]);
  console.log(JSON.stringify(output, null, 2));
}

function workerMain() {
  let values;
  let childWorkers = [];

  function childRequest(worker, payload) {
    return new Promise((resolve, reject) => {
      const errorListener = (error) => {
        worker.off("message", listener);
        reject(error);
      };
      const listener = (message) => {
        if (message.id !== payload.id) return;
        worker.off("message", listener);
        worker.off("error", errorListener);
        resolve(message);
      };
      worker.on("message", listener);
      worker.on("error", errorListener);
      worker.postMessage(payload);
    });
  }

  parentPort.on("message", (message) => {
    if (message.type === "init-coordinator") {
      values = new Uint32Array(message.buffer, 0, message.count);
      childWorkers = Array.from(
        { length: message.workers },
        () => new Worker(new URL(import.meta.url)),
      );
      void Promise.all(
        childWorkers.map((worker, index) =>
          childRequest(worker, {
            type: "init-shared",
            id: `${message.id}:${index}`,
            buffer: message.buffer,
            count: message.count,
          }),
        ),
      ).then(() => parentPort.postMessage({ id: message.id, ready: true }));
      return;
    }
    if (message.type === "shutdown") {
      void Promise.all(childWorkers.map((worker) => worker.terminate())).then(
        () => {
          parentPort.postMessage({ id: message.id, ready: true });
        },
      );
      return;
    }
    if (message.type === "init-shared") {
      values = new Uint32Array(message.buffer, 0, message.count);
      parentPort.postMessage({ id: message.id, ready: true });
      return;
    }
    if (message.type === "init-clone") {
      values = message.values;
      parentPort.postMessage({ id: message.id, ready: true });
      return;
    }
    if (message.type === "memory") {
      parentPort.postMessage({
        id: message.id,
        heapUsedMb: process.memoryUsage().heapUsed / 1024 / 1024,
      });
      return;
    }
    if (message.type === "work") {
      if (childWorkers.length > 0) {
        const width = Math.ceil(values.length / childWorkers.length);
        void Promise.all(
          childWorkers.map((worker, index) =>
            childRequest(worker, {
              type: "work",
              id: `${message.id}:${index}`,
              start: index * width,
              end: Math.min(values.length, (index + 1) * width),
              rounds: message.rounds,
            }),
          ),
        ).then((results) => {
          parentPort.postMessage({
            id: message.id,
            startedAt: Math.min(...results.map((result) => result.startedAt)),
            endedAt: Math.max(...results.map((result) => result.endedAt)),
            candidateCount: results.reduce(
              (sum, result) => sum + result.candidateCount,
              0,
            ),
            topK,
          });
        });
        return;
      }
      const startedAt = performance.now();
      const result = compute(
        values,
        message.start,
        message.end,
        message.rounds,
      );
      parentPort.postMessage({
        ...result,
        id: message.id,
        startedAt,
        endedAt: performance.now(),
      });
    }
  });
}

if (isMainThread) await main();
else workerMain();
