import type { ReactNode } from "react";
import { Dialog, DialogContent } from "../../components/ui/dialog";

export function ConnectionModal({
  children,
  labelledBy,
  onClose,
  size = "engine",
  underlay,
}: {
  children: ReactNode;
  labelledBy: string;
  onClose: () => void;
  size?: "config" | "engine";
  underlay: ReactNode;
}) {
  return (
    <div className="relative min-h-full overflow-hidden bg-app-content">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 select-none blur-sm opacity-30"
      >
        {underlay}
      </div>
      <Dialog open onOpenChange={(open) => !open && onClose()}>
        <DialogContent
          aria-labelledby={labelledBy}
          className="h-[min(620px,calc(100vh-4rem))] max-h-[calc(100vh-2rem)] max-w-none overflow-hidden bg-transparent p-0 shadow-none ring-0"
          style={{
            maxWidth: "none",
            minWidth: 0,
            width: `min(${size === "config" ? "820px" : "900px"}, calc(100vw - 3rem))`,
          }}
          overlayClassName="bg-overlay-backdrop backdrop-blur-xs"
          showCloseButton={false}
        >
          {children}
        </DialogContent>
      </Dialog>
    </div>
  );
}
