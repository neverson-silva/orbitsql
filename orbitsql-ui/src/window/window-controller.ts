import {
  getCurrentWindow,
  type Window as TauriWindow,
} from "@tauri-apps/api/window";
import { isTauriRuntime } from "./platform";

type Unlisten = () => void;

let currentWindow: TauriWindow | null = null;

function getWindow(): TauriWindow | null {
  if (!isTauriRuntime()) return null;
  currentWindow ??= getCurrentWindow();
  return currentWindow;
}

export const windowController = {
  async startDragging(): Promise<void> {
    const current = getWindow();
    console.log("[window-controller] startDragging", { available: !!current });
    await current?.startDragging();
  },

  async minimize(): Promise<void> {
    await getWindow()?.minimize();
  },

  async toggleMaximize(): Promise<void> {
    const current = getWindow();
    console.log("[window-controller] toggleMaximize:start", {
      available: !!current,
    });
    if (!current) return;
    const before = await current.isMaximized();
    await current.toggleMaximize();
    const after = await current.isMaximized();
    console.log("[window-controller] toggleMaximize:complete", {
      before,
      after,
    });
  },

  async close(): Promise<void> {
    await getWindow()?.close();
  },

  async toggleFullscreen(): Promise<void> {
    const current = getWindow();
    if (!current) return;
    await current.setFullscreen(!(await current.isFullscreen()));
  },

  async observeMaximized(
    onChange: (maximized: boolean) => void,
  ): Promise<Unlisten> {
    const current = getWindow();
    if (!current) {
      onChange(false);
      return () => undefined;
    }

    onChange(await current.isMaximized());
    return current.onResized(async () => {
      onChange(await current.isMaximized());
    });
  },
};
