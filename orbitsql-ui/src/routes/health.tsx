import { createFileRoute } from "@tanstack/react-router";
import { HealthTelemetryScreen } from "../features/database/health-telemetry";

export const Route = createFileRoute("/health")({
  component: HealthTelemetryScreen,
});
