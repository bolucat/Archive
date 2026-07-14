import "@fontsource-variable/schibsted-grotesk/index.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { TrayMenu } from "@dashboard/TrayMenu";
import { configurePreferenceStorage } from "@dashboard/lib/storage";
import "@dashboard/styles/globals.css";
import "@dashboard/styles/shared.css";

import { createDesktopHost } from "./host";

const desktop = createDesktopHost();
configurePreferenceStorage(desktop.preferences);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <TrayMenu desktop={desktop} />
  </StrictMode>,
);
