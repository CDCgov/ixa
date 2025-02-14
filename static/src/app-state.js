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
    api.getTime().then((time) => setCurrentTime(time));
  }, []);

  async function goNext() {
    let now = currentTime;
    setIsLoading(true);
    api.nextTime(currentTime + 1.0);
    // Busy wait on the API until time changes.
    while (now === currentTime) {
      now = await api.getTime();
    }
    setGeneration(generation + 1);
    setCurrentTime(now);
    setIsLoading(false);
  }

  let state = { generation, currentTime, goNext, isLoading, api };
  return html`<${SimulationContext.Provider} value=${state}>
    ${children}
  </${SimulationContext.Provider}>`;
}

export function useAppState() {
  return useContext(SimulationContext);
}
