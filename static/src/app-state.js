import { createContext, useContext, useEffect, useState } from "react";
import { html } from "htm/react";
import getApi from "./api.js";

const SimulationContext = createContext({
  generation: 0,
  currentTime: 0,
  goNext: () => {},
  isLoading: false,
  api: getApi(),
});

export function SimulationContextProvider({ children }) {
  const [generation, setGeneration] = useState(0);
  const [currentTime, setCurrentTime] = useState(0);
  const [isLoading, setIsLoading] = useState(false);
  let api = getApi();

  useEffect(() => {
    api.getTime().then((t) => setCurrentTime(t));
  }, [generation]);

  async function goNext() {
    setIsLoading(true);
    await api.nextTime(currentTime + 1.0);
    setIsLoading(false);
    setGeneration(generation + 1);
  }

  let state = { generation, currentTime, goNext, isLoading, api };
  return html`<${SimulationContext.Provider} value=${state}>
    ${children}
  </${SimulationContext.Provider}>`;
}

export function useAppState() {
  return useContext(SimulationContext);
}
