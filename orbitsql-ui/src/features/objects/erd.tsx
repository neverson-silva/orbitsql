import { Icon } from "../../components/ui/icons";
import {
  Button,
  IconButton,
  SegmentedTabs,
  StatusPill,
  Surface,
} from "../../components/ui/ui";

const tables = ["connection", "company", "payment_process", "active_users.sql"];

export function ErdScreen() {
  return (
    <div className="mx-auto flex h-full max-w-[1540px] flex-col gap-3 p-5 lg:p-7">
      <div className="flex min-w-0 items-center gap-1 overflow-x-auto border-b border-outline-variant/70 pb-1">
        {tables.map((table, index) => (
          <div
            className={`flex h-8 shrink-0 items-center gap-2 rounded-t-md border px-3 font-mono text-label-caps ${index === 0 ? "border-surface-divider border-b-surface-raised bg-surface-raised text-on-surface" : "border-transparent text-on-surface-variant"}`}
            key={table}
          >
            <Icon
              name={table.endsWith(".sql") ? "terminal" : "table"}
              size={12}
            />
            {table}
            <Icon name="close" size={11} />
          </div>
        ))}
        <IconButton label="Add object tab" name="plus" />
      </div>
      <div className="flex flex-wrap items-center gap-2 text-label-caps text-on-surface-variant">
        <span className="font-mono">erp_hub</span>
        <Icon name="chevron-right" size={12} />
        <span className="font-mono text-on-surface">conn.connection</span>
        <StatusPill tone="neutral">RELATIONSHIP VIEW</StatusPill>
        <div className="ml-auto flex items-center gap-2">
          <Button variant="secondary" icon="refresh">
            Refresh Layout
          </Button>
          <IconButton label="Open SQL" name="terminal" />
        </div>
      </div>
      <Surface className="flex min-h-[610px] flex-1 flex-col overflow-hidden">
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-outline-variant/70 px-4 py-3">
          <SegmentedTabs
            options={[
              { id: "diagram", label: "Diagram", icon: "diagram" },
              { id: "data", label: "Data", icon: "grid" },
              { id: "properties", label: "Properties", icon: "list" },
            ]}
            value="diagram"
            onChange={() => {}}
          />
          <div className="flex items-center gap-1 rounded-md border border-control-border bg-control-surface p-1">
            <IconButton label="Zoom out" name="minus" />
            <span className="w-10 text-center font-mono text-label-caps text-on-surface-variant">
              100%
            </span>
            <IconButton label="Zoom in" name="plus" />
            <IconButton label="Fit diagram" name="maximize" />
            <IconButton label="More diagram options" name="more" />
          </div>
        </div>
        <div className="ds-diagram-canvas relative min-h-0 flex-1 overflow-hidden">
          <svg
            aria-hidden="true"
            className="pointer-events-none absolute inset-0 h-full w-full"
            preserveAspectRatio="none"
            viewBox="0 0 1200 620"
          >
            <path
              d="M380 220 C470 220 480 150 550 150"
              fill="none"
              stroke="var(--ds-color-primary)"
              strokeDasharray="5 5"
              strokeWidth="1.5"
            />
            <path
              d="M380 390 C480 390 480 460 550 460"
              fill="none"
              stroke="var(--ds-color-secondary)"
              strokeWidth="1.5"
            />
            <path
              d="M820 210 C900 210 900 155 980 155"
              fill="none"
              stroke="var(--ds-color-tertiary)"
              strokeDasharray="5 5"
              strokeWidth="1.5"
            />
            <path
              d="M820 430 C905 430 900 475 980 475"
              fill="none"
              stroke="var(--ds-color-outline)"
              strokeWidth="1.5"
            />
          </svg>
          <div className="absolute left-[7%] top-[22%] w-64">
            <ErdTable
              title="public.erp_company"
              accent="primary"
              rows={[
                ["id", "uuid", "PK"],
                ["name", "varchar(255)", ""],
                ["is_active", "boolean", ""],
                ["created_at", "timestamptz", ""],
              ]}
            />
          </div>
          <div className="absolute left-[39%] top-[10%] w-72">
            <ErdTable
              title="conn.connection"
              accent="secondary"
              rows={[
                ["id", "int", "PK"],
                ["company_id", "int", "FK"],
                ["host", "nvarchar(255)", ""],
                ["erp_type", "nvarchar(50)", ""],
              ]}
            />
          </div>
          <div className="absolute left-[39%] top-[57%] w-72">
            <ErdTable
              title="public.tenant_auth"
              accent="tertiary"
              rows={[
                ["id", "uuid", "PK"],
                ["company_id", "int", "FK"],
                ["provider", "varchar(40)", ""],
                ["secret_ref", "text", ""],
              ]}
            />
          </div>
          <div className="absolute right-[6%] top-[20%] w-64">
            <ErdTable
              title="public.erp_organization"
              accent="primary"
              rows={[
                ["id", "uuid", "PK"],
                ["company_id", "int", "FK"],
                ["display_name", "text", ""],
                ["slug", "varchar(120)", ""],
              ]}
            />
          </div>
          <div className="absolute right-[5%] bottom-[16%] w-64">
            <ErdTable
              title="public.audit_log"
              accent="warning"
              rows={[
                ["id", "bigint", "PK"],
                ["actor_id", "uuid", "FK"],
                ["action", "text", ""],
                ["created_at", "timestamptz", ""],
              ]}
            />
          </div>
          <span className="absolute left-[34%] top-[27%] rounded-md bg-surface-raised px-2 py-1 font-mono text-label-caps text-primary shadow-sm">
            company_id → id
          </span>
          <span className="absolute left-[67%] top-[27%] rounded-md bg-surface-raised px-2 py-1 font-mono text-label-caps text-tertiary shadow-sm">
            company_id → id
          </span>
          <div className="absolute bottom-4 left-4 flex items-center gap-3 rounded-lg border border-surface-divider bg-surface-raised/90 px-3 py-2 font-mono text-label-caps text-on-surface-variant shadow-panel">
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-primary" />
              Foreign key
            </span>
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-secondary" />
              Dependency
            </span>
            <span className="flex items-center gap-1.5">
              <span className="h-2 w-2 rounded-full bg-warning" />
              Audit
            </span>
          </div>
          <div className="absolute bottom-4 right-4 h-24 w-36 rounded-md border border-surface-divider bg-surface-raised/90 p-2 shadow-panel">
            <div className="relative h-full border border-surface-divider bg-surface-subtle">
              <span className="absolute left-2 top-3 h-5 w-8 rounded-sm bg-primary/20" />
              <span className="absolute left-12 top-7 h-7 w-10 rounded-sm bg-secondary/20" />
              <span className="absolute right-2 top-3 h-5 w-8 rounded-sm bg-tertiary/20" />
              <span className="absolute bottom-2 left-1/2 h-1.5 w-8 -translate-x-1/2 rounded-md bg-primary" />
            </div>
          </div>
        </div>
      </Surface>
    </div>
  );
}

function ErdTable({
  title,
  accent,
  rows,
}: {
  title: string;
  accent: "primary" | "secondary" | "tertiary" | "warning";
  rows: string[][];
}) {
  const colors = {
    primary: "bg-primary",
    secondary: "bg-secondary",
    tertiary: "bg-tertiary",
    warning: "bg-warning",
  };
  return (
    <div className="overflow-hidden rounded-lg border border-surface-divider bg-surface-raised shadow-panel">
      <div className="flex items-center gap-2 border-b border-outline-variant/70 px-3 py-2">
        <span className={`h-2 w-2 rounded-full ${colors[accent]}`} />
        <span className="font-mono text-label-caps font-semibold text-on-surface">
          {title}
        </span>
        <Icon
          name="more"
          size={13}
          className="ml-auto text-on-surface-variant"
        />
      </div>
      <div>
        {rows.map((row) => (
          <div
            className="flex items-center gap-2 border-b border-outline-variant/40 px-3 py-1.5 font-mono text-label-caps last:border-0"
            key={row[0]}
          >
            <Icon
              name={
                row[2] === "PK" ? "key" : row[2] === "FK" ? "link" : "circle"
              }
              size={10}
              className={row[2] ? "text-primary" : "text-outline"}
            />
            <span className="flex-1 text-on-surface">{row[0]}</span>
            <span className="text-on-surface-variant">{row[1]}</span>
            <span className="w-5 text-right text-label-caps text-secondary">
              {row[2]}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
