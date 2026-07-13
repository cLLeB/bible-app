import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ProjectionView } from "./ProjectionView";
import { StageView } from "./StageView";
import { ReadyGate } from "./components/ReadyGate";
import "./App.css";

// Apply the saved operator-console theme before first paint (no flash).
if (localStorage.getItem("ui-theme") === "dark") {
  document.documentElement.classList.add("dark");
}

// All windows load this same index.html and render by their Tauri label.
function rootFor(label: string) {
  if (label === "projection") return <ProjectionView />;
  if (label === "stage") return <StageView />;
  return <App />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ReadyGate>{rootFor(getCurrentWindow().label)}</ReadyGate>
  </React.StrictMode>,
);
