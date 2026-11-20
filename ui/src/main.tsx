import React from "react";
import { createRoot } from "react-dom/client";
import "./design-system/styles.css";
import "./design-system/icons.ts";
import "./app.css";
import { App } from "./App.tsx";

createRoot(document.getElementById("root")!).render(<App />);
