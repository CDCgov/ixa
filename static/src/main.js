import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { html } from "htm/react";
import App from "./App.js";
import { SimulationContextProvider } from "./app-state.js";

createRoot(document.getElementById("root")).render(html`<${StrictMode}>
  <${SimulationContextProvider}>
    <${App} />
  </${SimulationContextProvider}>
</${StrictMode}>`);
