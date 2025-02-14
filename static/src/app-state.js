import { createContext, useContext, useEffect, useState } from "react";
import { html } from "htm/react";
import { getTime, nextTime } from "./api.js";

const SimulationContext = createContext({
  generation: 0,
  currentTime: 0,
  goNext: () => {},
  isLoading: false,
});

export function SimulationContextProvider({ children }) {
  const [generation, setGeneration] = useState(0);
  const [currentTime, setCurrentTime] = useState(0);
  const [isLoading, setIsLoading] = useState(false);

  useEffect(() => {
    getTime().then((t) => setCurrentTime(t));
  }, [generation]);

  async function goNext() {
    setIsLoading(true);
    await nextTime(currentTime + 1.0);
    setIsLoading(false);
    setGeneration(generation + 1);
  }

  let state = { generation, currentTime, goNext, isLoading };
  return html`<${SimulationContext.Provider} value=${state}>
    ${children}
  </${SimulationContext.Provider}>`;
}

export function useAppState() {
  return useContext(SimulationContext);
}
