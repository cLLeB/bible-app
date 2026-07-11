import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { ProjectionView } from "./ProjectionView";
import "./App.css";

// Both the operator and projection windows load this same index.html.
// Each renders based on its Tauri window label.
const isProjection = getCurrentWindow().label === "projection";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isProjection ? <ProjectionView /> : <App />}</React.StrictMode>,
);
