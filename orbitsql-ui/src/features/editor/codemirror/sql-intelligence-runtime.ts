import type {
  CatalogIndex,
  CatalogSnapshot,
} from "../../sql-intelligence/catalog";
import type {
  CompletionExecutionContext,
  CompletionResponse,
  SyntaxSnapshot,
} from "../../sql-intelligence/completion/types";
import type {
  RuntimeChange,
  RuntimeCompletionRequest,
  RuntimeMessage,
  RuntimeWorkerMessage,
} from "./sql-intelligence-runtime-protocol";

type PendingRequest = {
  request: RuntimeCompletionRequest;
  resolve: (response: CompletionResponse) => void;
  reject: (error: Error) => void;
  cancelled: boolean;
  cancelState?: Int32Array;
  cancel: () => void;
};

type CatalogRegistration = Readonly<{
  promise: Promise<void>;
  references: number;
}>;

export type RuntimeCompletionHandle = Readonly<{
  requestId: string;
  promise: Promise<CompletionResponse>;
  cancel: () => void;
}>;

export type SqlIntelligenceRuntime = Readonly<{
  catalogKey: string;
  registerCatalog: Promise<void>;
  openDocument: (
    documentId: string,
    documentVersion: number,
    text: string,
    dialect: CatalogSnapshot["dialect"],
  ) => void;
  applyDocumentChanges: (
    documentId: string,
    fromVersion: number,
    toVersion: number,
    changes: readonly RuntimeChange[],
  ) => void;
  closeDocument: (documentId: string) => void;
  complete: (request: {
    documentId: string;
    documentVersion: number;
    position: number;
    executionContext?: CompletionExecutionContext;
    syntax?: SyntaxSnapshot;
  }) => RuntimeCompletionHandle;
}>;

const workerAvailable = typeof globalThis.Worker === "function";
let sharedRuntime: SharedRuntime | undefined;
let requestSequence = 0;

function abortError(): Error {
  const error = new Error("Completion request was aborted");
  error.name = "AbortError";
  return error;
}

function catalogKey(snapshot: CatalogSnapshot): string {
  return `${snapshot.connectionId}:${snapshot.id}:${snapshot.version}`;
}

function nextRequestId(): string {
  requestSequence += 1;
  return `completion:${requestSequence}`;
}

function createCancelState():
  | {
      buffer: SharedArrayBuffer;
      state: Int32Array;
    }
  | undefined {
  if (typeof globalThis.SharedArrayBuffer !== "function") return undefined;
  try {
    const buffer = new SharedArrayBuffer(4);
    return { buffer, state: new Int32Array(buffer) };
  } catch {
    return undefined;
  }
}

class SharedRuntime {
  readonly worker: Worker;
  readonly catalogs = new Map<string, CatalogRegistration>();
  readonly pending = new Map<string, PendingRequest>();
  readonly activeByDocument = new Map<string, PendingRequest>();
  readonly queuedByDocument = new Map<string, PendingRequest>();

  constructor() {
    this.worker = new Worker(
      new URL("./sql-intelligence-runtime.worker.ts", import.meta.url),
      { type: "module" },
    );
    this.worker.addEventListener("message", (event) => {
      this.handleMessage(event.data as RuntimeWorkerMessage);
    });
    this.worker.addEventListener("error", (event) => {
      sharedRuntime = undefined;
      const error = new Error(
        event.message || "SQL intelligence worker failed",
      );
      for (const pending of this.pending.values()) pending.reject(error);
      this.pending.clear();
      this.activeByDocument.clear();
      for (const pending of this.queuedByDocument.values())
        pending.reject(error);
      this.queuedByDocument.clear();
    });
  }

  retainCatalog(snapshot: CatalogSnapshot): Promise<void> {
    const key = catalogKey(snapshot);
    const current = this.catalogs.get(key);
    if (current) {
      this.catalogs.set(key, {
        ...current,
        references: current.references + 1,
      });
      return current.promise;
    }
    const promise = new Promise<void>((resolve, reject) => {
      const listener = (event: MessageEvent<RuntimeWorkerMessage>) => {
        const message = event.data;
        if (message.type === "catalog-ready" && message.catalogKey === key) {
          this.worker.removeEventListener("message", listener);
          resolve();
        }
        if (
          message.type === "runtime-error" &&
          message.requestId === undefined &&
          message.message.startsWith(`${key}:`)
        ) {
          this.worker.removeEventListener("message", listener);
          this.catalogs.delete(key);
          reject(new Error(message.message));
        }
      };
      this.worker.addEventListener("message", listener);
      this.worker.postMessage({
        type: "register-catalog",
        catalogKey: key,
        snapshot,
      } satisfies RuntimeMessage);
    });
    this.catalogs.set(key, { promise, references: 1 });
    return promise;
  }

  releaseCatalog(key: string): void {
    const current = this.catalogs.get(key);
    if (!current) return;
    if (current.references > 1) {
      this.catalogs.set(key, {
        ...current,
        references: current.references - 1,
      });
      return;
    }
    this.catalogs.delete(key);
    this.worker.postMessage({
      type: "release-catalog",
      catalogKey: key,
    } satisfies RuntimeMessage);
  }

  cancelDocument(documentId: string): void {
    const active = this.activeByDocument.get(documentId);
    if (active) active.cancel();
    const queued = this.queuedByDocument.get(documentId);
    if (queued) this.cancelQueued(queued);
  }

  enqueue(pending: PendingRequest): void {
    if (pending.cancelled) return;
    const active = this.activeByDocument.get(pending.request.documentId);
    if (active) {
      const queued = this.queuedByDocument.get(pending.request.documentId);
      if (queued) this.cancelQueued(queued);
      this.queuedByDocument.set(pending.request.documentId, pending);
      active.cancel();
      return;
    }
    this.dispatch(pending);
  }

  dispatch(pending: PendingRequest): void {
    if (pending.cancelled) return;
    this.activeByDocument.set(pending.request.documentId, pending);
    this.pending.set(pending.request.requestId, pending);
    this.worker.postMessage(pending.request satisfies RuntimeMessage);
  }

  cancelQueued(pending: PendingRequest): void {
    pending.cancelled = true;
    pending.reject(abortError());
    this.queuedByDocument.delete(pending.request.documentId);
  }

  handleMessage(message: RuntimeWorkerMessage): void {
    if (message.type === "completion-result") {
      const pending = this.pending.get(message.requestId);
      if (!pending) return;
      this.pending.delete(message.requestId);
      this.activeByDocument.delete(pending.request.documentId);
      if (!pending.cancelled) pending.resolve(message.response);
      const queued = this.queuedByDocument.get(pending.request.documentId);
      if (queued) {
        this.queuedByDocument.delete(pending.request.documentId);
        this.dispatch(queued);
      }
      return;
    }
    if (message.type === "runtime-error" && message.requestId) {
      const pending = this.pending.get(message.requestId);
      if (!pending) return;
      this.pending.delete(message.requestId);
      this.activeByDocument.delete(pending.request.documentId);
      if (!pending.cancelled) pending.reject(new Error(message.message));
      const queued = this.queuedByDocument.get(pending.request.documentId);
      if (queued) {
        this.queuedByDocument.delete(pending.request.documentId);
        this.dispatch(queued);
      }
    }
  }
}

class RuntimeSession implements SqlIntelligenceRuntime {
  readonly catalogKey: string;
  readonly registerCatalog: Promise<void>;
  private closed = false;

  constructor(
    private readonly shared: SharedRuntime,
    snapshot: CatalogSnapshot,
  ) {
    this.catalogKey = catalogKey(snapshot);
    this.registerCatalog = shared.retainCatalog(snapshot);
  }

  openDocument(
    documentId: string,
    documentVersion: number,
    text: string,
    dialect: CatalogSnapshot["dialect"],
  ): void {
    this.shared.worker.postMessage({
      type: "open-document",
      documentId,
      documentVersion,
      catalogKey: this.catalogKey,
      dialect,
      text,
    } satisfies RuntimeMessage);
  }

  applyDocumentChanges(
    documentId: string,
    fromVersion: number,
    toVersion: number,
    changes: readonly RuntimeChange[],
  ): void {
    this.shared.worker.postMessage({
      type: "apply-document-changes",
      documentId,
      fromVersion,
      toVersion,
      changes,
    } satisfies RuntimeMessage);
  }

  closeDocument(documentId: string): void {
    if (this.closed) return;
    this.closed = true;
    this.shared.cancelDocument(documentId);
    this.shared.worker.postMessage({
      type: "close-document",
      documentId,
    } satisfies RuntimeMessage);
    this.shared.releaseCatalog(this.catalogKey);
  }

  complete(request: {
    documentId: string;
    documentVersion: number;
    position: number;
    executionContext?: CompletionExecutionContext;
    syntax?: SyntaxSnapshot;
  }): RuntimeCompletionHandle {
    const requestId = nextRequestId();
    const cancelState = createCancelState();
    const runtimeRequest: RuntimeCompletionRequest = {
      type: "complete",
      requestId,
      documentId: request.documentId,
      documentVersion: request.documentVersion,
      catalogKey: this.catalogKey,
      position: request.position,
      executionContext: request.executionContext,
      syntax: request.syntax,
      maxItems: 100,
      cancelBuffer: cancelState?.buffer,
    };
    let pending!: PendingRequest;
    const promise = new Promise<CompletionResponse>((resolve, reject) => {
      pending = {
        request: runtimeRequest,
        resolve,
        reject,
        cancelled: false,
        cancelState: cancelState?.state,
        cancel: () => {
          if (pending.cancelled) return;
          pending.cancelled = true;
          if (
            this.shared.queuedByDocument.get(request.documentId) === pending
          ) {
            this.shared.queuedByDocument.delete(request.documentId);
            reject(abortError());
            return;
          }
          if (pending.cancelState) Atomics.store(pending.cancelState, 0, 1);
          this.shared.worker.postMessage({
            type: "cancel",
            requestId,
          } satisfies RuntimeMessage);
          reject(abortError());
        },
      };
    });
    void this.registerCatalog
      .then(() => this.shared.enqueue(pending))
      .catch((error: unknown) => {
        if (!pending.cancelled) {
          pending.cancelled = true;
          pending.reject(
            error instanceof Error ? error : new Error(String(error)),
          );
        }
      });
    return { requestId, promise, cancel: pending.cancel };
  }
}

export function createSqlIntelligenceRuntime(
  catalog: CatalogIndex,
): SqlIntelligenceRuntime | undefined {
  if (!workerAvailable) return undefined;
  if (!sharedRuntime) {
    try {
      sharedRuntime = new SharedRuntime();
    } catch {
      return undefined;
    }
  }
  return new RuntimeSession(sharedRuntime, catalog.snapshot);
}

export function runtimeCatalogKey(snapshot: CatalogSnapshot): string {
  return catalogKey(snapshot);
}
