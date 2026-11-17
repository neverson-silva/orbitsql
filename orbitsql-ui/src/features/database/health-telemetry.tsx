import { Icon } from "../../components/ui/icons";
import { ScreenHeader, Surface } from "../../components/ui/ui";

export function HealthTelemetryScreen() {
  return (
    <div className="mx-auto flex max-w-[1500px] flex-col gap-4 p-5 lg:p-7">
      <ScreenHeader
        eyebrow="Database health"
        title="Health &amp; Telemetry"
        description="Live operational signals from the active database connection."
      />
      <Surface className="p-8">
        <div className="flex min-h-64 flex-col items-center justify-center text-center">
          <Icon name="activity" size={28} className="mb-3 text-primary" />
          <h2 className="text-body-md font-semibold text-on-surface">
            Health telemetry is not available yet
          </h2>
          <p className="mt-2 max-w-xl text-body-sm text-on-surface-variant">
            The runtime does not expose database health metrics yet. This view
            will be populated from live driver data when that provider is
            implemented.
          </p>
        </div>
      </Surface>
    </div>
  );
}
