import { createContext, useContext, useEffect, useState } from "react";
import { getTime, nextTime } from "./api";

interface SimulationContextData {
    generation: number;
    currentTime: number;
    goNext: () => void;
}

const SimulationContext = createContext<SimulationContextData>({
    generation: 0,
    currentTime: 0,
    goNext: () => {},
});

export function SimulationContextProvider({
    children,
}: {
    children: React.ReactNode;
}) {
    const [generation, setGeneration] = useState(0);
    const [currentTime, setCurrentTime] = useState(0);

    useEffect(() => {
        getTime().then((t) => setCurrentTime(t));
    }, [generation]);

    async function goNext() {
        nextTime(currentTime + 1.0)
        setGeneration(generation + 1);
    }

    return (
        <SimulationContext.Provider value={{ generation, currentTime, goNext }}>
            {children}
        </SimulationContext.Provider>
    );
}

export function useSimulation(): SimulationContextData {
    return useContext(SimulationContext)
}
