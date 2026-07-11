import React from "react";
import ReactDOM from "react-dom/client";
import { ProjectionView } from "./ProjectionView";
import "./App.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ProjectionView />
  </React.StrictMode>,
);
