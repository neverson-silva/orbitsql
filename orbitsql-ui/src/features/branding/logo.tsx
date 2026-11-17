import { OrbitLogo } from "../../components/ui/orbit-logo";
import { ScreenHeader, Surface } from "../../components/ui/ui";

export function LogoScreen() {
  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-5 p-5 lg:p-12">
      <ScreenHeader
        eyebrow="Brand asset / Precision Glass"
        title="Orbit SQL Logo"
        description="The database orbit mark is rendered as a theme-aware inline vector so it follows the active design system."
      />
      <div className="grid gap-3 sm:grid-cols-3">
        <LogoCard label="Compact" size={32} />
        <LogoCard label="Product" size={64} />
        <LogoCard label="Large" size={128} />
      </div>
    </div>
  );
}

function LogoCard({ label, size }: { label: string; size: number }) {
  return (
    <Surface className="flex min-h-44 flex-col items-center justify-center gap-4 p-5">
      <OrbitLogo size={size} />
      <span className="font-mono text-label-caps uppercase ds-tracking-caps-wide text-on-surface-variant">
        {label} / {size}px
      </span>
    </Surface>
  );
}
