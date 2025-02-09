import * as Plot from "@observablehq/plot";
import { useEffect, useRef, useState } from "react";
import { tabulateProperties } from "./api";
import { useSimulation } from "./useGeneration";

interface PeopleGraphProps {
    properties: string[];
}
interface HistoryItem {
    time: number, value: string, series: string
}
export function PeopleGraph({ properties }: PeopleGraphProps) {
    const plotRef = useRef<HTMLDivElement>(null);
    const [history, setHistory] = useState<HistoryItem[]>([]);
    const { currentTime, generation } = useSimulation();
    useEffect(() => {
        (async () => {
            const results = await tabulateProperties(properties);
            const newHistoryItems: HistoryItem[] = [];
            for (const [propMap, value] of results) {
                const label = [];
                for (const prop of properties) {
                    label.push(`${prop}:${propMap[prop]}`);
                }
                newHistoryItems.push( {
                    time: currentTime,
                    value,
                    series: label.join("|"),
                });
            }
            setHistory(h => [...h, ...newHistoryItems]);
        })();
    }, [properties, currentTime, generation]);

    useEffect(() => {
        const plot = Plot.plot({
            color: { legend: true },
            marks: [
                Plot.line(history, {
                    x: "time",
                    y: "value",
                    stroke: "series",
                }),
            ],
        });
        plotRef.current?.replaceChildren(plot);
    }, [history]);

    return <div className="people-graph">
        <b>Time Series</b>
        <div ref={plotRef}></div>
    </div>;
}
