import "./PeopleCharts.css";
import { useEffect, useRef, useState } from "react";
import { useSimulation } from "./useGeneration";
import { getPeoplePropertiesList, tabulateProperties } from "./api";
import * as Plot from "@observablehq/plot";
import { Serializable } from "./App";

export function PeopleChartsContainer() {
    const { generation } = useSimulation();
    const [properties, setProperties] = useState<string[]>([]);
    const [selectedProperties, setSelectedProperties] = useState<string[]>([]);
    useEffect(() => {
        getPeoplePropertiesList().then((p) => setProperties(p));
    }, [generation]);

    if (!properties.length) {
        return "Loading...";
    }
    const handleCheck = (p: string, isChecked: boolean) => {
        if (isChecked && !selectedProperties.includes(p)) {
            setSelectedProperties([...selectedProperties, p]);
        } else if (!isChecked) {
            setSelectedProperties(
                selectedProperties.filter((existing) => existing !== p)
            );
        }
    };
    return (
        <>
            <div className="properties-checkboxes">
                {properties.map((p) => (
                    <label htmlFor={`people-container-${p}`}>
                        <input
                            id={`people-container-${p}`}
                            type="checkbox"
                            value={p}
                            onChange={(e) => handleCheck(p, e.target.checked)}
                        />
                        {p}
                    </label>
                ))}
            </div>
            {selectedProperties.length ? (
                <>
                    <PeopleTable properties={selectedProperties} />
                    <PeopleGraph properties={selectedProperties} />
                </>
            ) : (
                <p>Select at least one property to see a visualization.</p>
            )}
        </>
    );
}

interface PeopleGraphProps {
    properties: string[];
}
interface HistoryItem {
    time: number, value: string, series: string
}
interface History {
    property_key: string;
    data: HistoryItem[];
}
export function PeopleGraph({ properties }: PeopleGraphProps) {
    const plotRef = useRef<HTMLDivElement>(null);
    const [history, setHistory] = useState<History>({
        property_key: "",
        data: [],
    });
    const { currentTime, generation } = useSimulation();
    const property_key = properties.join("");

    useEffect(() => {
        (async () => {
            const results = await tabulateProperties(properties);
            const newHistoryItems: HistoryItem[] = [];
            for (const [propMap, value] of results) {
                const label = [];
                for (const prop of properties) {
                    label.push(`${prop}:${propMap[prop]}`);
                }
                newHistoryItems.push({
                    time: currentTime,
                    value,
                    series: label.join("|"),
                });
            }
            setHistory((h) => {
                // Only modify the existing history if the properties haven't changed
                if (h.property_key === property_key) {
                    return { ...h, data: [...h.data, ...newHistoryItems] };
                } else {
                    return {
                        property_key,
                        data: newHistoryItems,
                    };
                }
            });
        })();
    }, [generation, property_key]);

    useEffect(() => {
        const plot = Plot.plot({
            color: { legend: true },
            marks: [
                Plot.line(history.data, {
                    x: "time",
                    y: "value",
                    stroke: "series",
                }),
            ],
        });
        plotRef.current?.replaceChildren(plot);
    }, [history]);

    return (
        <div className="people-graph">
            <h3>Timeseries</h3>
            <div className="chart-container" ref={plotRef}></div>
        </div>
    );
}

type Row = {key: string, data: Serializable[]};

export function PeopleTable({ properties }: { properties: string[] }) {
    const { generation, currentTime } = useSimulation();
    const [tabulated, setTabulated] = useState<Row[]>([]);
    useEffect(() => {
        (async () => {
            const result = await tabulateProperties(properties);
            const tableRows: Row[] = [];
            for (const row of result) {
                const columns = properties.map((prop) => row[0][prop]);
                console.log(columns);
                const key = columns.join("|");
                columns.push(row[1]);
                tableRows.push({ key, data: columns});
            }
            setTabulated(tableRows);
        })();
    }, [generation, properties.join("|")]);

    const headerColumns = [...properties, "Count"];

    return <div>
        <h3>People Status at t = {currentTime.toFixed(1)}</h3>
        <table className="table">
            <thead>
                <tr>
                    {headerColumns.map((prop) => <th key={prop}>{prop}</th>)}
                </tr>
            </thead>
            <tbody>
                {tabulated.map(r => <tr key={r.key}>{r.data.map((c, i) => <td key={i}>{c}</td>)}</tr>)}
            </tbody>
        </table>
    </div>
}
