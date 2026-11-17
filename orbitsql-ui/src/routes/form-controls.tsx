import { createFileRoute } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { useState } from "react";
import {
  Combobox,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "../components/ui/combobox";
import { Input } from "../components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "../components/ui/native-select";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Textarea } from "../components/ui/textarea";
import { Toggle } from "../components/ui/toggle";
import type { Theme, ThemeSurfaceRoles } from "../design-system";
import {
  applyDesignSystem,
  formatSurfaceMetric,
  getSurfaceDiagnosticRows,
  resolveSurfaceRoles,
  themeRegistry,
} from "../design-system";
import { useSettingsStore } from "../stores/settings.store";

export const Route = createFileRoute("/form-controls")({
  component: FormControlsPlayground,
});

const selectOptions = ["PostgreSQL", "MySQL", "MariaDB"];

function ControlLabel({ children }: { children: string }) {
  return (
    <p className="mb-1.5 text-label-caps font-medium text-on-surface-variant">
      {children}
    </p>
  );
}

const surfaceRoleLabels: Array<{
  key: keyof ThemeSurfaceRoles;
  label: string;
}> = [
  { key: "appChrome", label: "Chrome" },
  { key: "appSidebar", label: "Sidebar" },
  { key: "appToolbar", label: "Toolbar" },
  { key: "appWorkspace", label: "Editor / Results" },
  { key: "surfaceRaised", label: "Raised" },
  { key: "controlSurface", label: "Control" },
  { key: "surfaceFloating", label: "Floating" },
];

function ThemeSurfaceRow({ theme }: { theme: Theme }) {
  const roles = resolveSurfaceRoles(theme);
  const diagnosticRows = getSurfaceDiagnosticRows(theme);
  const roleMetrics = new Map(
    diagnosticRows.map((row) => [row.role, row.oklchLightness]),
  );

  return (
    <article className="rounded border border-surface-divider bg-surface-base p-3">
      <div className="mb-2 flex items-baseline justify-between gap-2">
        <h3 className="text-label-md font-semibold">{theme.name}</h3>
        <span className="text-label-caps uppercase text-on-surface-variant">
          {theme.type}
        </span>
      </div>
      <div className="grid grid-cols-2 gap-1.5 sm:grid-cols-4 lg:grid-cols-7">
        {surfaceRoleLabels.map(({ key, label }) => (
          <div className="min-w-0" key={key}>
            <div
              className="h-8 rounded border border-outline-variant/60"
              style={{ backgroundColor: roles[key] }}
              title={`${label}: ${roles[key]}`}
            />
            <p className="mt-1 truncate text-[9px] text-on-surface-variant">
              {label}
            </p>
            <p className="font-mono text-[9px] text-on-surface-variant">
              L {formatSurfaceMetric(roleMetrics.get(key) ?? null)}
            </p>
          </div>
        ))}
      </div>
    </article>
  );
}

function FormControlsPlayground() {
  const settings = useSettingsStore();
  const [selectedTheme, setSelectedTheme] = useState(settings.themeId);
  const [selectedCombo, setSelectedCombo] = useState("PostgreSQL");
  const [grayscale, setGrayscale] = useState(false);

  function previewTheme(themeId: string) {
    setSelectedTheme(themeId);
    applyDesignSystem({ ...settings, themeId });
  }

  return (
    <main
      className={`min-h-full overflow-y-auto bg-surface-base p-5 text-on-surface ${grayscale ? "grayscale" : ""}`}
    >
      <div className="mx-auto flex max-w-5xl flex-col gap-6">
        <header className="border-b border-outline-variant/70 pb-4">
          <p className="text-label-caps font-medium text-primary">
            Design system playground
          </p>
          <h1 className="mt-1 text-headline-md font-semibold">Form controls</h1>
          <p className="mt-2 max-w-2xl text-body-sm text-on-surface-variant">
            Shared control surfaces, borders and interaction states across
            runtime themes.
          </p>
        </header>

        <section className="rounded border border-outline-variant/70 bg-surface-floating p-4 shadow-panel">
          <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
            <div>
              <h2 className="text-body-md font-semibold">Theme matrix</h2>
              <p className="mt-1 text-body-sm text-on-surface-variant">
                Preview the same primitives against every registered theme.
              </p>
            </div>
            <NativeSelect
              aria-label="Preview theme"
              className="min-w-48"
              onChange={(event) => previewTheme(event.target.value)}
              value={selectedTheme}
            >
              {themeRegistry.getAll().map((theme) => (
                <NativeSelectOption key={theme.id} value={theme.id}>
                  {theme.name}
                </NativeSelectOption>
              ))}
            </NativeSelect>
            <button
              className="h-8 rounded border border-control-border px-2.5 text-label-caps text-control-foreground transition-colors hover:border-control-border-hover hover:bg-control-surface-hover"
              onClick={() => setGrayscale((value) => !value)}
              type="button"
            >
              {grayscale ? "Color preview" : "Grayscale preview"}
            </button>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {themeRegistry.getAll().map((theme) => (
              <button
                className="rounded border border-control-border px-2 py-1 text-label-caps text-control-foreground transition-colors hover:border-control-border-hover hover:bg-control-surface-hover data-[active=true]:border-control-border-focus data-[active=true]:bg-control-surface-active"
                data-active={theme.id === selectedTheme}
                key={theme.id}
                onClick={() => previewTheme(theme.id)}
                type="button"
              >
                {theme.name}
              </button>
            ))}
          </div>
        </section>

        <section className="rounded border border-surface-divider bg-surface-subtle p-4">
          <div className="mb-4">
            <h2 className="text-body-md font-semibold">Surface debug</h2>
            <p className="mt-1 text-body-sm text-on-surface-variant">
              Structural roles use palette planes; state variations stay derived
              from their semantic parent.
            </p>
          </div>
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
            <SurfaceSwatch label="Chrome" className="bg-app-chrome" />
            <SurfaceSwatch label="Content" className="bg-app-content" />
            <SurfaceSwatch label="Sidebar" className="bg-app-sidebar" />
            <SurfaceSwatch label="Workspace" className="bg-app-workspace" />
            <SurfaceSwatch
              label="Workspace subtle"
              className="bg-app-workspace-subtle"
            />
            <SurfaceSwatch label="Control" className="bg-control-surface" />
            <SurfaceSwatch label="Floating" className="bg-floating-surface" />
            <SurfaceSwatch label="Selected" className="bg-selected-surface" />
            <SurfaceSwatch label="Divider" className="bg-surface-divider" />
          </div>
          <div className="mt-4 grid gap-2 md:grid-cols-5">
            <StateSwatch label="Default" className="bg-control-surface" />
            <StateSwatch
              label="Hover"
              className="border-control-border-hover bg-control-surface-hover"
            />
            <StateSwatch
              label="Selected"
              className="border-control-border-focus bg-selected-surface"
            />
            <StateSwatch
              label="Focus"
              className="border-control-border-focus bg-control-surface-focus ring-1 ring-control-focus-ring/40"
            />
            <StateSwatch
              label="Disabled"
              className="border-control-border-disabled bg-control-surface-disabled text-control-foreground-disabled"
            />
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-2">
            <Toggle aria-label="Preview selected control" variant="outline">
              Selected control
            </Toggle>
            <Toggle aria-label="Preview active control" variant="outline">
              Active control
            </Toggle>
          </div>
        </section>

        <section className="rounded border border-surface-divider bg-surface-subtle p-4">
          <div className="mb-4">
            <h2 className="text-body-md font-semibold">Theme surface matrix</h2>
            <p className="mt-1 text-body-sm text-on-surface-variant">
              Structural planes use the active theme palette. Editor and results
              intentionally share the primary work plane.
            </p>
          </div>
          <div className="grid gap-2">
            {themeRegistry.getAll().map((theme) => (
              <ThemeSurfaceRow key={theme.id} theme={theme} />
            ))}
          </div>
        </section>

        <section className="grid gap-5 lg:grid-cols-2">
          <ControlPanel title="Input">
            <ControlLabel>Default</ControlLabel>
            <Input defaultValue="Active value" />
            <ControlLabel>Hover / focus / invalid</ControlLabel>
            <Input aria-invalid="true" defaultValue="Validation message" />
            <ControlLabel>Disabled</ControlLabel>
            <Input defaultValue="Unavailable value" disabled />
            <ControlLabel>Readonly</ControlLabel>
            <Input defaultValue="Readable value" readOnly />
          </ControlPanel>

          <ControlPanel title="Select">
            <ControlLabel>Default / hover / focus</ControlLabel>
            <Select defaultValue={selectOptions[0]}>
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {selectOptions.map((option) => (
                  <SelectItem key={option} value={option}>
                    {option}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <ControlLabel>Open</ControlLabel>
            <Select
              defaultValue={selectOptions[1]}
              open
              onOpenChange={() => undefined}
            >
              <SelectTrigger className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {selectOptions.map((option) => (
                  <SelectItem key={option} value={option}>
                    {option}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <ControlLabel>Disabled / invalid</ControlLabel>
            <Select defaultValue={selectOptions[2]} disabled>
              <SelectTrigger aria-invalid="true" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {selectOptions.map((option) => (
                  <SelectItem key={option} value={option}>
                    {option}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </ControlPanel>

          <ControlPanel title="Textarea">
            <ControlLabel>Default / focus</ControlLabel>
            <Textarea defaultValue="The field stays in the same tonal family as its container." />
            <ControlLabel>Disabled</ControlLabel>
            <Textarea
              defaultValue="Disabled controls remain legible."
              disabled
            />
            <ControlLabel>Readonly / invalid</ControlLabel>
            <Textarea
              aria-invalid="true"
              defaultValue="Readonly content"
              readOnly
            />
          </ControlPanel>

          <ControlPanel title="Combobox">
            <ControlLabel>Input + floating listbox</ControlLabel>
            <Combobox
              items={selectOptions}
              onValueChange={(value) => setSelectedCombo(String(value ?? ""))}
              value={selectedCombo}
            >
              <ComboboxInput
                aria-label="Database engine"
                showClear
                showTrigger
              />
              <ComboboxContent>
                <ComboboxList>
                  {selectOptions.map((option) => (
                    <ComboboxItem key={option} value={option}>
                      {option}
                    </ComboboxItem>
                  ))}
                </ComboboxList>
              </ComboboxContent>
            </Combobox>
            <p className="text-body-sm text-on-surface-variant">
              Keyboard focus, selection and hover share the control roles.
            </p>
          </ControlPanel>
        </section>
      </div>
    </main>
  );
}

function ControlPanel({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <section className="flex flex-col gap-2.5 rounded border border-outline-variant/70 bg-surface-floating p-4 shadow-panel">
      <h2 className="text-body-md font-semibold">{title}</h2>
      {children}
    </section>
  );
}

function SurfaceSwatch({
  className,
  label,
}: {
  className: string;
  label: string;
}) {
  return (
    <div className="overflow-hidden rounded border border-surface-divider">
      <div className={`h-14 ${className}`} />
      <p className="border-t border-surface-divider px-2 py-1.5 text-label-caps text-on-surface-variant">
        {label}
      </p>
    </div>
  );
}

function StateSwatch({
  className,
  label,
}: {
  className: string;
  label: string;
}) {
  return (
    <div
      className={`flex h-9 items-center rounded border px-2 text-label-caps text-control-foreground ${className}`}
    >
      {label}
    </div>
  );
}
