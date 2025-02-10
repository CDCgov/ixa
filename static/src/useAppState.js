import { createContext, useContext, useEffect, useState } from "react";
import { html } from "htm/react";
import { getTime, nextTime } from "../api.js";

const SimulationContext = createContext({
  generation: 0,
  currentTime: 0,
  goNext: () => {},
});

export function SimulationContextProvider({ children }) {
  const [generation, setGeneration] = useState(0);
  const [currentTime, setCurrentTime] = useState(0);

  useEffect(() => {
    getTime().then((t) => setCurrentTime(t));
  }, [generation]);

  async function goNext() {
    nextTime(currentTime + 1.0);
    setGeneration(generation + 1);
  }

  let state = { generation, currentTime, goNext };
  return html`<${SimulationContext.Provider} value=${state}>
    ${children}
  </${SimulationContext.Provider}>`;
}

export function useAppState() {
  return useContext(SimulationContext);
}
