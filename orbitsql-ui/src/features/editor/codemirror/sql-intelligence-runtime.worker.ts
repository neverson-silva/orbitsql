import type {
  CatalogIndex,
  CatalogSnapshot,
} from "../../sql-intelligence/catalog";
import { buildCatalogIndex } from "../../sql-intelligence/catalog";
import { complete } from "../../sql-intelligence/completion/engine";
import { createSqlDocument } from "../../sql-intelligence/document/factory";
import type { RuntimeMessage } from "./sql-intelligence-runtime-protocol";
import { applyRuntimeChanges } from "./sql-intelligence-runtime-protocol";

type WorkerDocument = {
  version: number;
  text: string;
  dialect: CatalogSnapshot["dialect"];
  catalogKey: string;
};

const catalogs = new Map<string, CatalogIndex>();
const documents = new Map<string, WorkerDocument>();
const controllers = new Map<
  string,
  { controller: AbortController; cancelState?: Int32Array }
>();
const workerGlobal = globalThis as unknown as {
  addEventListener: (
    type: "message",
    listener: (event: MessageEvent<RuntimeMessage>) => void,
  ) => void;
  postMessage: (message: unknown) => void;
};

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function runCompletion(
  message: Extract<RuntimeMessage, { type: "complete" }>,
): Promise<void> {
  const documentState = documents.get(message.documentId);
  const catalog = catalogs.get(message.catalogKey);
  if (!documentState || !catalog) {
    workerGlobal.postMessage({
      type: "runtime-error",
      requestId: message.requestId,
      message: "Document or catalog is not registered",
    });
    return;
  }
  if (documentState.version !== message.documentVersion) {
    workerGlobal.postMessage({
      type: "runtime-error",
      requestId: message.requestId,
      message: "Document version is stale",
    });
    return;
  }
  const controller = new AbortController();
  const cancelState = message.cancelBuffer
    ? new Int32Array(message.cancelBuffer)
    : undefined;
  controllers.set(message.requestId, { controller, cancelState });
  const signal = cancelState
    ? ({
        get aborted() {
          return (
            controller.signal.aborted || Atomics.load(cancelState, 0) === 1
          );
        },
      } as AbortSignal)
    : controller.signal;
  try {
    const document = createSqlDocument(
      message.documentId,
      message.documentVersion,
      documentState.text,
      documentState.dialect,
    );
    const response = complete({
      document,
      catalog,
      position: message.position,
      executionContext: message.executionContext,
      signal,
      syntax: message.syntax,
      maxItems: message.maxItems,
    });
    workerGlobal.postMessage({
      type: "completion-result",
      requestId: message.requestId,
      response,
    });
  } catch (error) {
    workerGlobal.postMessage({
      type: "runtime-error",
      requestId: message.requestId,
      message: errorMessage(error),
    });
  } finally {
    controllers.delete(message.requestId);
  }
}

workerGlobal.addEventListener("message", (event) => {
  const message = event.data;
  if (message.type === "register-catalog") {
    try {
      catalogs.set(message.catalogKey, buildCatalogIndex(message.snapshot));
      workerGlobal.postMessage({
        type: "catalog-ready",
        catalogKey: message.catalogKey,
      });
    } catch (error) {
      workerGlobal.postMessage({
        type: "runtime-error",
        message: `${message.catalogKey}:${errorMessage(error)}`,
      });
    }
    return;
  }
  if (message.type === "release-catalog") {
    catalogs.delete(message.catalogKey);
    return;
  }
  if (message.type === "open-document") {
    documents.set(message.documentId, {
      version: message.documentVersion,
      text: message.text,
      dialect: message.dialect,
      catalogKey: message.catalogKey,
    });
    return;
  }
  if (message.type === "apply-document-changes") {
    const document = documents.get(message.documentId);
    if (!document || document.version !== message.fromVersion) return;
    documents.set(message.documentId, {
      ...document,
      version: message.toVersion,
      text: applyRuntimeChanges(document.text, message.changes),
    });
    return;
  }
  if (message.type === "close-document") {
    documents.delete(message.documentId);
    return;
  }
  if (message.type === "cancel") {
    const request = controllers.get(message.requestId);
    if (request?.cancelState) Atomics.store(request.cancelState, 0, 1);
    request?.controller.abort();
    return;
  }
  if (message.type === "complete") void runCompletion(message);
});
