import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { html } from "htm/react";
import App from "./src/App.js";
import { SimulationContextProvider } from "./src/useAppState.js";

createRoot(document.getElementById("root")).render(html`<${StrictMode}>
  <${SimulationContextProvider}>
    <${App} />
  </${SimulationContextProvider}>
</${StrictMode}>`);
