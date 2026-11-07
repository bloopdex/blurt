import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/bricolage-grotesque";

import "./index.css";
import { getCurrentWindow } from "@tauri-apps/api/window";

import App from "./App";
import { Overlay } from "./views/Overlay";

// Desktop app: the browser's own context menu (Reload, Inspect…) has no place here.
if (import.meta.env.PROD) window.addEventListener("contextmenu", (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {getCurrentWindow().label === "overlay" ? <Overlay /> : <App />}
  </React.StrictMode>,
);
