import { createFileRoute } from "@tanstack/react-router";
import { LogoScreen } from "../features/branding/logo";

export const Route = createFileRoute("/logo")({
  component: LogoScreen,
});
