import { useNavigate } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";
import { APP_NAME, APP_VERSION_LABEL } from "../../app-meta";
import { Dialog, DialogContent, DialogTitle } from "../../components/ui/dialog";
import { Icon } from "../../components/ui/icons";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../components/ui/select";
import { Switch } from "../../components/ui/switch";
import { Button, IconButton, StatusPill } from "../../components/ui/ui";
import {
  applyDesignSystem,
  DEFAULT_DESIGN_SETTINGS,
  DENSITY_OPTIONS,
  type Density,
  type DesignSettings,
  EDITOR_FONT_FAMILIES,
  EDITOR_FONT_SIZE_RANGE,
  themeRegistry,
  UI_FONT_FAMILIES,
  UI_FONT_SIZE_RANGE,
} from "../../design-system";
import { useSettingsStore } from "../../stores/settings.store";
import { QueryWorkspaceScreen } from "../query/query-workspace";

const sections = [
  ["General", "settings"],
  ["Appearance", "tune"],
  ["Execution & Safety", "shield"],
  ["AI Local LLM", "bolt"],
  ["Security & Keychain", "key"],
  ["Data Grid & Export", "grid"],
  ["Drivers & Network", "network"],
  ["Shortcuts & Keymap", "terminal"],
] as const;

export function SettingsModalScreen() {
  const navigate = useNavigate();
  const [activeSection, setActiveSection] = useState("General");
  const [initialDesignSettings] = useState<DesignSettings>(() => {
    const state = useSettingsStore.getState();
    return {
      themeId: state.themeId,
      uiFontFamily: state.uiFontFamily,
      uiFontSize: state.uiFontSize,
      editorFontFamily: state.editorFontFamily,
      editorFontSize: state.editorFontSize,
      density: state.density,
      radius: state.radius,
    };
  });
  const [draftDesignSettings, setDraftDesignSettings] =
    useState<DesignSettings>(initialDesignSettings);
  const setSidebarOpen = useSettingsStore((state) => state.setSidebarOpen);
  const applyDesignSettings = useSettingsStore(
    (state) => state.applyDesignSettings,
  );
  const closeSettings = () => {
    applyDesignSystem(initialDesignSettings);
    navigate({ to: "/query" });
  };
  const updateDraftDesignSettings = (updated: Partial<DesignSettings>) => {
    const nextSettings = { ...draftDesignSettings, ...updated };
    setDraftDesignSettings(nextSettings);
    applyDesignSystem(nextSettings);
  };

  return (
    <div className="relative h-full">
      <QueryWorkspaceScreen embedded />
      <Dialog open onOpenChange={(open) => !open && closeSettings()}>
        <DialogContent
          aria-labelledby="application-settings-title"
          className="h-[min(780px,calc(100vh-3rem))] max-h-[calc(100vh-3rem)] max-w-[980px] overflow-hidden bg-transparent p-0 shadow-none ring-0 sm:max-w-[980px]"
          overlayBlur={false}
          overlayClassName="bg-overlay-backdrop"
          showCloseButton={false}
        >
          <div className="flex h-full max-h-[calc(100vh-3rem)] w-full max-w-[980px] flex-col overflow-hidden rounded-lg border border-floating-border bg-dialog-surface text-on-surface shadow-overlay">
            <div className="flex items-center justify-between border-b border-outline-variant/70 px-5 py-3">
              <div>
                <p className="font-mono text-label-caps uppercase ds-tracking-caps-wide text-primary">
                  Workspace preferences
                </p>
                <DialogTitle
                  className="text-headline-sm font-semibold"
                  id="application-settings-title"
                >
                  Application Settings
                </DialogTitle>
              </div>
              <IconButton
                label="Close settings"
                name="close"
                onClick={closeSettings}
              />
            </div>
            <div className="grid min-h-0 flex-1 md:grid-cols-[205px_1fr]">
              <aside className="border-b border-outline-variant/70 bg-dialog-sidebar-surface p-2 md:border-b-0 md:border-r">
                <p className="px-2 pb-2 pt-1 font-mono text-label-caps uppercase ds-tracking-caps-wide text-on-surface-variant">
                  Preferences
                </p>
                <nav className="space-y-0.5">
                  {sections.map(([label, icon]) => (
                    <Button
                      variant="ghost"
                      className={`flex w-full items-center justify-start gap-2 rounded-md px-2.5 py-2 text-left text-label-caps transition-colors ${activeSection === label ? "bg-selected-surface font-medium text-on-surface" : "text-on-surface-variant hover:bg-surface-subtle hover:text-on-surface"}`}
                      key={label}
                      onClick={() => setActiveSection(label)}
                      type="button"
                    >
                      <Icon name={icon} size={13} />
                      {label}
                    </Button>
                  ))}
                </nav>
                <div className="mt-4 border-t border-outline-variant/60 px-2 pt-3 font-mono text-label-caps text-on-surface-variant">
                  <div className="flex items-center gap-1.5 text-success">
                    <span className="h-1.5 w-1.5 rounded-full bg-success" />
                    Settings synced locally
                  </div>
                  <p className="mt-1">
                    {APP_NAME} {APP_VERSION_LABEL}
                  </p>
                </div>
              </aside>
              <div className="min-h-0 min-w-0 overflow-auto bg-dialog-content-surface p-5">
                {activeSection === "General" ? (
                  <GeneralSettings setSidebarOpen={setSidebarOpen} />
                ) : activeSection === "Appearance" ? (
                  <AppearanceSettings
                    settings={draftDesignSettings}
                    onChange={updateDraftDesignSettings}
                  />
                ) : (
                  <PlaceholderSettings
                    title={activeSection}
                    icon={
                      sections.find(
                        ([label]) => label === activeSection,
                      )?.[1] ?? "settings"
                    }
                  />
                )}
              </div>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant/70 bg-transparent px-5 py-3">
              <Button
                variant="ghost"
                className="text-label-caps text-error hover:underline"
                onClick={() => {
                  setDraftDesignSettings(DEFAULT_DESIGN_SETTINGS);
                  applyDesignSystem(DEFAULT_DESIGN_SETTINGS);
                }}
                type="button"
              >
                Reset default
              </Button>
              <div className="flex items-center gap-2">
                <Button variant="ghost" onClick={closeSettings}>
                  Cancel
                </Button>
                <Button
                  variant="primary"
                  icon="check"
                  onClick={() => {
                    applyDesignSettings(draftDesignSettings);
                    navigate({ to: "/query" });
                  }}
                >
                  Apply Changes
                </Button>
              </div>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function GeneralSettings({
  setSidebarOpen,
}: {
  setSidebarOpen: (value: boolean) => void;
}) {
  const sidebarOpen = useSettingsStore((state) => state.sidebarOpen);
  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-body-lg font-semibold">General Workspace</h2>
        <p className="mt-1 text-body-sm text-on-surface-variant">
          Configure how Orbit SQL behaves across sessions and surfaces.
        </p>
      </div>
      <SettingGroup title="Interface">
        <ToggleRow
          title="Strict Light Architecture"
          detail="Use high-contrast surfaces and avoid translucent content panels."
          checked={true}
        />
        <ToggleRow
          title="Show Navigation Sidebar"
          detail="Keep the connection tree visible beside the workspace."
          checked={sidebarOpen}
          onChange={setSidebarOpen}
        />
      </SettingGroup>
      <SettingGroup title="Startup">
        <div className="grid gap-3 sm:grid-cols-2">
          <SelectField
            label="On application startup"
            value="Open Connection Hub"
            options={[
              "Open Connection Hub",
              "Restore last workspace",
              "Open empty workspace",
            ]}
          />
          <SelectField
            label="Default connection scope"
            value="PostgreSQL Prod"
            options={[
              "PostgreSQL Prod",
              "Last active connection",
              "No connection",
            ]}
          />
        </div>
      </SettingGroup>
      <div className="grid gap-3 sm:grid-cols-2">
        <SafetyCard
          title="Safe Mode"
          detail="Prevent destructive statements until explicitly confirmed."
          tone="warning"
        />
        <SafetyCard
          title="Auto-Commit Transactions"
          detail="Keep transaction boundaries visible and manually controlled."
          tone="success"
        />
      </div>
    </div>
  );
}

function AppearanceSettings({
  settings,
  onChange,
}: {
  settings: DesignSettings;
  onChange: (updated: Partial<DesignSettings>) => void;
}) {
  const {
    themeId,
    uiFontFamily,
    uiFontSize,
    editorFontFamily,
    editorFontSize,
    density,
    radius,
  } = settings;
  const themes = [...themeRegistry.getAll()].sort((left, right) =>
    left.name.localeCompare(right.name, undefined, { sensitivity: "base" }),
  );
  if (themes.length === 0) return null;

  return (
    <div className="min-w-0 max-w-4xl space-y-5">
      <div>
        <h2 className="text-body-lg font-semibold">Appearance</h2>
        <p className="mt-1 text-body-sm text-on-surface-variant">
          Tune the visual language independently for the interface and editor.
        </p>
      </div>
      <SettingGroup title="Theme Engine">
        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(180px,220px)]">
          <SelectField
            label="Application theme"
            value={themeId}
            options={themes.map((theme) => ({
              label: theme.name,
              value: theme.id,
            }))}
            onChange={(value) => onChange({ themeId: value })}
            contentClassName="!max-h-64 !w-[260px] overflow-y-auto"
          />
          <SelectField
            label="Corner radius"
            value={String(radius)}
            options={["0", "4", "8", "12", "16"]}
            onChange={(value) => onChange({ radius: Number(value) })}
          />
        </div>
        <div className="mt-3 grid gap-2 sm:grid-cols-3">
          {themes.slice(0, 6).map((theme) => (
            <Button
              variant="ghost"
              className={`rounded-md border p-2 text-left ${theme.id === themeId ? "border-primary bg-primary/5" : "border-outline-variant/60 bg-transparent"}`}
              key={theme.id}
              onClick={() => onChange({ themeId: theme.id })}
              type="button"
            >
              <span className="mb-2 flex gap-1">
                <i
                  className="h-4 flex-1 rounded-sm"
                  style={{ backgroundColor: theme.colors.primary }}
                />
                <i
                  className="h-4 flex-1 rounded-sm"
                  style={{ backgroundColor: theme.colors.secondary }}
                />
                <i
                  className="h-4 flex-1 rounded-sm"
                  style={{ backgroundColor: theme.colors.surface }}
                />
              </span>
              <span className="block truncate text-label-caps font-medium">
                {theme.name}
              </span>
              <span className="font-mono text-label-caps text-on-surface-variant">
                {theme.type}
              </span>
            </Button>
          ))}
        </div>
      </SettingGroup>
      <SettingGroup title="UI Typography">
        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(180px,220px)]">
          <SelectField
            label="Interface font"
            value={uiFontFamily}
            options={UI_FONT_FAMILIES.map((font) => font.value)}
            onChange={(value) => onChange({ uiFontFamily: value })}
          />
          <SelectField
            label="UI size"
            value={String(uiFontSize)}
            options={Array.from(
              {
                length: UI_FONT_SIZE_RANGE.max - UI_FONT_SIZE_RANGE.min + 1,
              },
              (_, index) => String(UI_FONT_SIZE_RANGE.min + index),
            ).map((size) => ({
              label: `${String(size)}px`,
              value: size,
            }))}
            onChange={(value) => onChange({ uiFontSize: Number(value) })}
          />
        </div>
      </SettingGroup>
      <SettingGroup title="UI Density">
        <SelectField
          label="UI density"
          value={density}
          options={DENSITY_OPTIONS.map((option) => ({
            label: option.name,
            value: option.id,
          }))}
          onChange={(value) => onChange({ density: value as Density })}
        />
      </SettingGroup>
      <SettingGroup title="Editor Typography">
        <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_minmax(180px,220px)]">
          <SelectField
            label="Editor font"
            value={editorFontFamily}
            options={EDITOR_FONT_FAMILIES.map((font) => font.value)}
            onChange={(value) => onChange({ editorFontFamily: value })}
          />
          <SelectField
            label="Editor size"
            value={String(editorFontSize)}
            options={Array.from(
              {
                length:
                  EDITOR_FONT_SIZE_RANGE.max - EDITOR_FONT_SIZE_RANGE.min + 1,
              },
              (_, index) => String(EDITOR_FONT_SIZE_RANGE.min + index),
            ).map((size) => ({
              label: `${String(size)}px`,
              value: size,
            }))}
            onChange={(value) => onChange({ editorFontSize: Number(value) })}
          />
        </div>
      </SettingGroup>
    </div>
  );
}

function PlaceholderSettings({
  title,
  icon,
}: {
  title: string;
  icon:
    | "settings"
    | "tune"
    | "code"
    | "shield"
    | "bolt"
    | "key"
    | "grid"
    | "network"
    | "terminal";
}) {
  return (
    <div className="flex min-h-80 flex-col items-center justify-center text-center">
      <span className="mb-3 flex h-11 w-11 items-center justify-center rounded-xl bg-primary/10 text-primary">
        <Icon name={icon} size={22} />
      </span>
      <h2 className="text-body-lg font-semibold">{title}</h2>
      <p className="mt-1 max-w-md text-body-sm text-on-surface-variant">
        This preference group is represented in the frontend shell and ready for
        a future provider. No backend or connection state is changed.
      </p>
      <StatusPill className="mt-4" tone="neutral">
        Frontend only
      </StatusPill>
    </div>
  );
}

function SettingGroup({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <section className="border-b border-outline-variant/60 pb-5 last:border-b-0">
      <h3 className="mb-2 font-mono text-label-caps font-semibold uppercase ds-tracking-caps-wide text-primary">
        {title}
      </h3>
      {children}
    </section>
  );
}

function ToggleRow({
  title,
  detail,
  checked,
  onChange,
}: {
  title: string;
  detail: string;
  checked: boolean;
  onChange?: (value: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-outline-variant/60 py-2.5 last:border-0 last:pb-0">
      <div>
        <p className="text-label-md font-medium">{title}</p>
        <p className="mt-1 text-label-caps text-on-surface-variant">{detail}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
  contentClassName,
}: {
  label: string;
  value: string;
  options: Array<string | { label: string; value: string }>;
  onChange?: (value: string) => void;
  contentClassName?: string;
}) {
  const selectedOption = options.find(
    (option) => (typeof option === "string" ? option : option.value) === value,
  );
  const selectedLabel =
    selectedOption === undefined
      ? value
      : typeof selectedOption === "string"
        ? selectedOption
        : selectedOption.label;

  return (
    <div className="block">
      <span className="mb-1.5 block text-label-caps font-medium text-on-surface-variant">
        {label}
      </span>
      <Select
        aria-label={label}
        onValueChange={(next) => {
          if (next !== null) onChange?.(next);
        }}
        value={value}
      >
        <SelectTrigger className="w-full text-label-caps">
          <SelectValue>{selectedLabel}</SelectValue>
        </SelectTrigger>
        <SelectContent className={contentClassName}>
          {options.map((option) => {
            const optionValue =
              typeof option === "string" ? option : option.value;
            const optionLabel =
              typeof option === "string" ? option : option.label;
            return (
              <SelectItem key={optionValue} value={optionValue}>
                {optionLabel}
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>
    </div>
  );
}

function SafetyCard({
  title,
  detail,
  tone,
}: {
  title: string;
  detail: string;
  tone: "warning" | "success";
}) {
  return (
    <div className="rounded-md border border-outline-variant/60 bg-transparent p-3">
      <div className="flex items-center gap-2">
        <Icon
          name={tone === "warning" ? "shield" : "verified"}
          size={14}
          className={tone === "warning" ? "text-warning" : "text-success"}
        />
        <span className="text-label-md font-medium">{title}</span>
        <StatusPill tone={tone}>Enabled</StatusPill>
      </div>
      <p className="mt-2 text-label-caps text-on-surface-variant">{detail}</p>
    </div>
  );
}
