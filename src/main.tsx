import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import UpdaterApp from "./UpdaterApp";

const isUpdater = getCurrentWindow().label === "updater";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isUpdater ? <UpdaterApp /> : <App />}
  </React.StrictMode>,
);