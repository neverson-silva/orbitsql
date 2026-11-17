import { useSyncExternalStore } from "react";
import { COMMAND_IDS, type CommandId } from "./command-model";

export type CommandHandler = () => void | Promise<void>;

export type CommandState = {
  enabled?: boolean;
  checked?: boolean;
  visible?: boolean;
};

export type CommandStateSnapshot = {
  enabled: boolean;
  checked?: boolean;
  visible: boolean;
};

const handlers = new Map<CommandId, CommandHandler>();
const states = new Map<CommandId, CommandState>();
const listeners = new Set<() => void>();
let version = 0;

function notify(): void {
  version += 1;
  for (const listener of listeners) listener();
}

export function registerCommandHandlers(
  nextHandlers: Partial<Record<CommandId, CommandHandler>>,
  nextStates: Partial<Record<CommandId, CommandState>> = {},
): () => void {
  const registered = Object.entries(nextHandlers) as [
    CommandId,
    CommandHandler,
  ][];
  const registeredStates = Object.entries(nextStates) as [
    CommandId,
    CommandState,
  ][];
  const previous = new Map<CommandId, CommandHandler | undefined>();
  const previousStates = new Map<CommandId, CommandState | undefined>();

  for (const [id, handler] of registered) {
    previous.set(id, handlers.get(id));
    handlers.set(id, handler);
  }
  for (const [id, state] of registeredStates) {
    previousStates.set(id, states.get(id));
    states.set(id, state);
  }
  notify();

  return () => {
    for (const [id, handler] of registered) {
      if (handlers.get(id) !== handler) continue;
      const prior = previous.get(id);
      if (prior) handlers.set(id, prior);
      else handlers.delete(id);
    }
    for (const [id, state] of registeredStates) {
      if (states.get(id) !== state) continue;
      const prior = previousStates.get(id);
      if (prior) states.set(id, prior);
      else states.delete(id);
    }
    notify();
  };
}

export function isCommandAvailable(id: CommandId): boolean {
  return handlers.has(id);
}

export function getCommandState(id: CommandId): CommandStateSnapshot {
  const state = states.get(id);
  return {
    enabled: handlers.has(id) && state?.enabled !== false,
    checked: state?.checked,
    visible: state?.visible !== false,
  };
}

export function setCommandState(id: CommandId, state: CommandState): void {
  states.set(id, state);
  notify();
}

export function getCommandStateSnapshot(): Record<
  CommandId,
  CommandStateSnapshot
> {
  const commandIds = new Set<CommandId>([
    ...Object.values(COMMAND_IDS),
    ...handlers.keys(),
    ...states.keys(),
  ]);
  return Object.fromEntries(
    [...commandIds].map((id) => [id, getCommandState(id)]),
  ) as Record<CommandId, CommandStateSnapshot>;
}

export async function executeCommand(id: CommandId): Promise<boolean> {
  const handler = handlers.get(id);
  if (!handler) return false;
  await handler();
  return true;
}

export function subscribeCommandRegistry(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useCommandAvailability(id: CommandId): boolean {
  return useSyncExternalStore(
    subscribeCommandRegistry,
    () => isCommandAvailable(id),
    () => false,
  );
}

export function useCommandState(id: CommandId): CommandStateSnapshot {
  const enabled = useSyncExternalStore(
    subscribeCommandRegistry,
    () => getCommandState(id).enabled,
    () => false,
  );
  const checked = useSyncExternalStore(
    subscribeCommandRegistry,
    () => getCommandState(id).checked,
    () => undefined,
  );
  const visible = useSyncExternalStore(
    subscribeCommandRegistry,
    () => getCommandState(id).visible,
    () => true,
  );
  return { enabled, checked, visible };
}

export function useCommandRegistryVersion(): number {
  return useSyncExternalStore(
    subscribeCommandRegistry,
    () => version,
    () => 0,
  );
}
