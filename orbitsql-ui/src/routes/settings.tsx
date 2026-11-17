import { createFileRoute } from "@tanstack/react-router";
import { SettingsModalScreen } from "../features/settings/settings-modal";

export const Route = createFileRoute("/settings")({
  component: SettingsModalScreen,
});
