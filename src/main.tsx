import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ProjectionView } from "./ProjectionView";
import { StageView } from "./StageView";
import "./App.css";

// All windows load this same index.html and render by their Tauri label.
function rootFor(label: string) {
  if (label === "projection") return <ProjectionView />;
  if (label === "stage") return <StageView />;
  return <App />;
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{rootFor(getCurrentWindow().label)}</React.StrictMode>,
);
