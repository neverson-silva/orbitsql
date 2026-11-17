import {
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  clampSidebarWidth,
  SIDEBAR_WIDTH,
  useSettingsStore,
} from "../stores/settings.store";

type DragState = {
  currentWidth: number;
  element: HTMLElement;
  pointerId: number;
  startX: number;
  startWidth: number;
};

type ResizePointerEvent = {
  clientX: number;
  pointerId: number;
};

export function useResizableSidebar() {
  const sidebarOpen = useSettingsStore((state) => state.sidebarOpen);
  const sidebarWidth = useSettingsStore((state) => state.sidebarWidth);
  const setSidebarWidth = useSettingsStore((state) => state.setSidebarWidth);
  const [isResizing, setIsResizing] = useState(false);
  const [draftWidth, setDraftWidth] = useState<number | null>(null);
  const dragState = useRef<DragState | null>(null);

  const stopResizing = useCallback(() => {
    const currentDrag = dragState.current;
    if (currentDrag) {
      setSidebarWidth(currentDrag.currentWidth);
      if (currentDrag.element.hasPointerCapture(currentDrag.pointerId)) {
        currentDrag.element.releasePointerCapture(currentDrag.pointerId);
      }
    }
    dragState.current = null;
    setDraftWidth(null);
    setIsResizing(false);
  }, [setSidebarWidth]);

  const startResizing = useCallback(
    (event: ReactPointerEvent<HTMLElement>) => {
      if (!sidebarOpen || event.button !== 0) return;
      dragState.current = {
        currentWidth: sidebarWidth,
        element: event.currentTarget,
        pointerId: event.pointerId,
        startX: event.clientX,
        startWidth: sidebarWidth,
      };
      setDraftWidth(sidebarWidth);
      event.currentTarget.setPointerCapture(event.pointerId);
      setIsResizing(true);
    },
    [sidebarOpen, sidebarWidth],
  );

  const resize = useCallback((event: ResizePointerEvent) => {
    const currentDrag = dragState.current;
    if (!currentDrag || currentDrag.pointerId !== event.pointerId) return;
    const nextWidth = clampSidebarWidth(
      currentDrag.startWidth + event.clientX - currentDrag.startX,
    );
    currentDrag.currentWidth = nextWidth;
    setDraftWidth(nextWidth);
  }, []);

  useEffect(() => {
    if (!isResizing) return;
    const handlePointerMove = (event: globalThis.PointerEvent) => {
      resize(event);
    };
    const handlePointerEnd = (event: globalThis.PointerEvent) => {
      if (dragState.current?.pointerId !== event.pointerId) return;
      stopResizing();
    };
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerEnd);
    window.addEventListener("pointercancel", handlePointerEnd);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerEnd);
      window.removeEventListener("pointercancel", handlePointerEnd);
    };
  }, [isResizing, resize, stopResizing]);

  const resizeWithKeyboard = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (!sidebarOpen) return;
      const step = event.shiftKey ? 32 : 16;
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        setSidebarWidth(sidebarWidth - step);
      }
      if (event.key === "ArrowRight") {
        event.preventDefault();
        setSidebarWidth(sidebarWidth + step);
      }
      if (event.key === "Home") {
        event.preventDefault();
        setSidebarWidth(SIDEBAR_WIDTH.min);
      }
      if (event.key === "End") {
        event.preventDefault();
        setSidebarWidth(SIDEBAR_WIDTH.max);
      }
    },
    [sidebarOpen, sidebarWidth, setSidebarWidth],
  );

  useEffect(() => {
    if (!isResizing) return;
    const previousUserSelect = document.body.style.userSelect;
    const previousCursor = document.body.style.cursor;
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";
    return () => {
      document.body.style.userSelect = previousUserSelect;
      document.body.style.cursor = previousCursor;
    };
  }, [isResizing]);

  return {
    isResizing,
    sidebarWidth: draftWidth ?? sidebarWidth,
    startResizing,
    resize,
    stopResizing,
    resizeWithKeyboard,
  };
}
