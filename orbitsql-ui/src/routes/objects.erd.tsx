import { createFileRoute } from "@tanstack/react-router";
import { ErdScreen } from "../features/objects/erd";

export const Route = createFileRoute("/objects/erd")({
  component: ErdScreen,
});
