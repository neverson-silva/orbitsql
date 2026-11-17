import { useSidebarFolderStore } from "../../stores/sidebar.store";
import { Icon } from "../ui/icons";

function HeaderButton({
  label,
  icon,
  onClick,
}: {
  label: string;
  icon: "folder" | "plus";
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className="flex size-7 items-center justify-center rounded-md text-on-surface-variant hover:bg-app-chrome-subtle hover:text-on-surface focus-visible:outline-none"
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon name={icon} size={15} />
    </button>
  );
}

export function SidebarHeader({
  onAddConnection,
}: {
  onAddConnection: () => void;
}) {
  const openNewFolder = useSidebarFolderStore((state) => state.openNewFolder);

  return (
    <header
      className="flex h-full min-w-0 items-center justify-between px-2"
      data-slot="sidebar-header"
    >
      <span className="min-w-0 truncate font-mono text-label-caps font-semibold uppercase tracking-[0.2em] text-on-surface-variant">
        Connections
      </span>
      <div className="flex shrink-0 items-center gap-0.5">
        <HeaderButton
          icon="folder"
          label="New folder"
          onClick={openNewFolder}
        />
        <HeaderButton
          icon="plus"
          label="Add connection"
          onClick={onAddConnection}
        />
      </div>
    </header>
  );
}
