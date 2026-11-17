import { useLocation, useNavigate } from "@tanstack/react-router";
import { Icon, type IconName } from "../ui/icons";

type RailAction = {
  label: string;
  icon: IconName;
  path: string;
};

const railActions: readonly RailAction[] = [
  { label: "Query editor", icon: "terminal", path: "/query" },
  { label: "Connections", icon: "database", path: "/connections" },
  { label: "Schema explorer", icon: "table", path: "/schema" },
  { label: "Sessions", icon: "activity", path: "/session" },
  {
    label: "Entity relationship diagram",
    icon: "diagram",
    path: "/objects/erd",
  },
];

function RailButton({
  action,
  active,
  onClick,
}: {
  action: RailAction;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      aria-current={active ? "page" : undefined}
      aria-label={action.label}
      className="relative flex size-8 items-center justify-center rounded-md text-on-surface-variant transition-colors hover:bg-app-chrome-subtle hover:text-on-surface focus-visible:bg-app-chrome-subtle focus-visible:outline-none"
      onClick={onClick}
      title={action.label}
      type="button"
    >
      {active ? (
        <span
          aria-hidden="true"
          className="absolute inset-y-2 start-0 w-0.5 rounded-e-full bg-primary"
        />
      ) : null}
      <Icon name={action.icon} size={16} />
    </button>
  );
}

export function ActivityRail({
  onToggleExplorer,
  explorerOpen,
}: {
  onToggleExplorer: () => void;
  explorerOpen: boolean;
}) {
  const location = useLocation();
  const navigate = useNavigate();

  return (
    <nav
      aria-label="Primary tools"
      className={`col-start-1 box-border flex h-full min-h-0 min-w-0 flex-col items-center bg-transparent py-2 ${explorerOpen ? "border-r border-structural-divider" : "border-r-0"}`}
      data-slot="sidebar-activity-rail"
    >
      <div className="flex flex-col items-center gap-1">
        {railActions.map((action) => {
          const active =
            location.pathname === action.path ||
            location.pathname.startsWith(`${action.path}/`);
          return (
            <RailButton
              action={action}
              active={active}
              key={action.path}
              onClick={() => navigate({ to: action.path })}
            />
          );
        })}
      </div>
      <div className="mt-auto flex flex-col items-center gap-1">
        <button
          aria-label={explorerOpen ? "Collapse sidebar" : "Expand sidebar"}
          className="flex size-8 items-center justify-center rounded-md text-on-surface-variant transition-colors hover:bg-app-chrome-subtle hover:text-on-surface focus-visible:bg-app-chrome-subtle focus-visible:outline-none"
          onClick={onToggleExplorer}
          title={explorerOpen ? "Collapse sidebar" : "Expand sidebar"}
          type="button"
        >
          <Icon name="dock" size={16} />
        </button>
        <button
          aria-label="Open application settings"
          className="flex size-8 items-center justify-center rounded-md text-on-surface-variant transition-colors hover:bg-app-chrome-subtle hover:text-on-surface focus-visible:bg-app-chrome-subtle focus-visible:outline-none"
          onClick={() => navigate({ to: "/settings" })}
          title="Open application settings"
          type="button"
        >
          <Icon name="settings" size={16} />
        </button>
      </div>
    </nav>
  );
}
