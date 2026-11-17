import { createRouter, RouterProvider } from "@tanstack/react-router";
import ReactDOM from "react-dom/client";
import { applyDesignSystem } from "./design-system";
import { routeTree } from "./routeTree.gen";
import { hydrateSettings, useSettingsStore } from "./stores/settings.store";
import "./styles.css";

const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  scrollRestoration: true,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("OrbitSQL root element was not found");
}

const reactRoot = ReactDOM.createRoot(rootElement);

async function bootstrap(): Promise<void> {
  await hydrateSettings();
  applyDesignSystem(useSettingsStore.getState());
  reactRoot.render(<RouterProvider router={router} />);
}

void bootstrap();
