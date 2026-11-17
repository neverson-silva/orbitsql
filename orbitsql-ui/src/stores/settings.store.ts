import { load } from "@tauri-apps/plugin-store";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  type StateStorage,
} from "zustand/middleware";
import {
  applyDesignSystem,
  DEFAULT_DESIGN_SETTINGS,
  type Density,
  type DesignSettings,
} from "../design-system";
import { isTauriRuntime } from "../window/platform";

export type SettingsState = DesignSettings & {
  sidebarOpen: boolean;
  sidebarWidth: number;
  zoomPercent: number;
  setSidebarOpen: (sidebarOpen: boolean) => void;
  setSidebarWidth: (sidebarWidth: number) => void;
  toggleSidebar: () => void;
  setThemeId: (themeId: string) => void;
  setUiFontFamily: (uiFontFamily: string) => void;
  setUiFontSize: (uiFontSize: number) => void;
  setEditorFontFamily: (editorFontFamily: string) => void;
  setEditorFontSize: (editorFontSize: number) => void;
  setDensity: (density: Density) => void;
  setRadius: (radius: number) => void;
  setZoomPercent: (zoomPercent: number) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  resetZoom: () => void;
  applyDesignSettings: (settings: DesignSettings) => void;
  resetDesignSettings: () => void;
};

export const UI_ZOOM = {
  min: 50,
  max: 200,
  step: 10,
  default: 100,
} as const;

export function clampZoomPercent(zoomPercent: number): number {
  return Math.min(
    UI_ZOOM.max,
    Math.max(
      UI_ZOOM.min,
      Math.round(zoomPercent / UI_ZOOM.step) * UI_ZOOM.step,
    ),
  );
}

function applyUiZoom(zoomPercent: number): void {
  if (typeof document === "undefined") return;
  document.documentElement.style.zoom = `${zoomPercent}%`;
}

export const SIDEBAR_WIDTH = {
  default: 232,
  min: 208,
  max: 360,
} as const;

export function clampSidebarWidth(sidebarWidth: number): number {
  return Math.min(
    SIDEBAR_WIDTH.max,
    Math.max(SIDEBAR_WIDTH.min, Math.round(sidebarWidth)),
  );
}

const memoryStorage = new Map<string, string>();
const tauriStorePromise = isTauriRuntime()
  ? load("settings.json", { autoSave: false })
  : null;

const settingsStorage: StateStorage = {
  getItem: async (name) => {
    if (!tauriStorePromise) {
      return memoryStorage.get(name) ?? null;
    }

    const store = await tauriStorePromise;
    return (await store.get<string>(name)) ?? null;
  },
  setItem: async (name, value) => {
    if (!tauriStorePromise) {
      memoryStorage.set(name, value);
      return;
    }

    const store = await tauriStorePromise;
    await store.set(name, value);
    await store.save();
  },
  removeItem: async (name) => {
    if (!tauriStorePromise) {
      memoryStorage.delete(name);
      return;
    }

    const store = await tauriStorePromise;
    await store.delete(name);
    await store.save();
  },
};

function designSettingsFromState(state: SettingsState): DesignSettings {
  return {
    themeId: state.themeId,
    uiFontFamily: state.uiFontFamily,
    uiFontSize: state.uiFontSize,
    editorFontFamily: state.editorFontFamily,
    editorFontSize: state.editorFontSize,
    density: state.density,
    radius: state.radius,
  };
}

function applyUpdatedSettings(
  get: () => SettingsState,
  updated: Partial<DesignSettings>,
): void {
  applyDesignSystem({ ...designSettingsFromState(get()), ...updated });
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      ...DEFAULT_DESIGN_SETTINGS,
      sidebarOpen: true,
      sidebarWidth: SIDEBAR_WIDTH.default,
      zoomPercent: UI_ZOOM.default,
      setSidebarOpen: (sidebarOpen) => set({ sidebarOpen }),
      setSidebarWidth: (sidebarWidth) =>
        set({ sidebarWidth: clampSidebarWidth(sidebarWidth) }),
      toggleSidebar: () =>
        set((state) => ({ sidebarOpen: !state.sidebarOpen })),
      setThemeId: (themeId) => {
        set({ themeId });
        applyUpdatedSettings(get, { themeId });
      },
      setUiFontFamily: (uiFontFamily) => {
        set({ uiFontFamily });
        applyUpdatedSettings(get, { uiFontFamily });
      },
      setUiFontSize: (uiFontSize) => {
        set({ uiFontSize });
        applyUpdatedSettings(get, { uiFontSize });
      },
      setEditorFontFamily: (editorFontFamily) => {
        set({ editorFontFamily });
        applyUpdatedSettings(get, { editorFontFamily });
      },
      setEditorFontSize: (editorFontSize) => {
        set({ editorFontSize });
        applyUpdatedSettings(get, { editorFontSize });
      },
      setDensity: (density) => {
        set({ density });
        applyUpdatedSettings(get, { density });
      },
      setRadius: (radius) => {
        set({ radius });
        applyUpdatedSettings(get, { radius });
      },
      setZoomPercent: (zoomPercent) => {
        const nextZoomPercent = clampZoomPercent(zoomPercent);
        set({ zoomPercent: nextZoomPercent });
        applyUiZoom(nextZoomPercent);
      },
      zoomIn: () => get().setZoomPercent(get().zoomPercent + UI_ZOOM.step),
      zoomOut: () => get().setZoomPercent(get().zoomPercent - UI_ZOOM.step),
      resetZoom: () => get().setZoomPercent(UI_ZOOM.default),
      applyDesignSettings: (settings) => {
        set({ ...settings });
        applyDesignSystem(settings);
      },
      resetDesignSettings: () => {
        set({ ...DEFAULT_DESIGN_SETTINGS });
        applyUpdatedSettings(get, DEFAULT_DESIGN_SETTINGS);
      },
    }),
    {
      name: "orbitsql.settings",
      storage: createJSONStorage(() => settingsStorage),
      skipHydration: true,
      version: 6,
      migrate: (persistedState) => {
        const state = persistedState as Partial<SettingsState>;
        return {
          ...DEFAULT_DESIGN_SETTINGS,
          ...state,
          themeId:
            state.themeId === "precision-glass-studio"
              ? DEFAULT_DESIGN_SETTINGS.themeId
              : (state.themeId ?? DEFAULT_DESIGN_SETTINGS.themeId),
          editorFontSize:
            state.editorFontSize === 13 || state.editorFontSize === 14
              ? DEFAULT_DESIGN_SETTINGS.editorFontSize
              : (state.editorFontSize ??
                DEFAULT_DESIGN_SETTINGS.editorFontSize),
          sidebarWidth: clampSidebarWidth(
            state.sidebarWidth ?? SIDEBAR_WIDTH.default,
          ),
          zoomPercent: clampZoomPercent(state.zoomPercent ?? UI_ZOOM.default),
        };
      },
      partialize: (state) => ({
        sidebarOpen: state.sidebarOpen,
        sidebarWidth: state.sidebarWidth,
        zoomPercent: state.zoomPercent,
        themeId: state.themeId,
        uiFontFamily: state.uiFontFamily,
        uiFontSize: state.uiFontSize,
        editorFontFamily: state.editorFontFamily,
        editorFontSize: state.editorFontSize,
        density: state.density,
        radius: state.radius,
      }),
    },
  ),
);

export async function hydrateSettings(): Promise<SettingsState> {
  try {
    await useSettingsStore.persist.rehydrate();
  } catch {
    useSettingsStore.setState({
      ...DEFAULT_DESIGN_SETTINGS,
      zoomPercent: UI_ZOOM.default,
    });
  }
  applyUiZoom(useSettingsStore.getState().zoomPercent);
  return useSettingsStore.getState();
}
