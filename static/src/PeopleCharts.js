import { useEffect, useRef, useState, Fragment } from "react";
import { html } from "htm/react";
import { useAppState } from "./useAppState.js";
import { getPeoplePropertiesList, tabulateProperties } from "./api.js";
import * as Plot from "@observablehq/plot";

export function PeopleChartsContainer() {
  const { generation } = useAppState();
  const [properties, setProperties] = useState([]);
  const [selectedProperties, setSelectedProperties] = useState([]);
  useEffect(() => {
    getPeoplePropertiesList().then((p) => setProperties(p));
  }, [generation]);

  if (!properties.length) {
    return "Loading...";
  }
  const handleCheck = (p, isChecked) => {
    if (isChecked && !selectedProperties.includes(p)) {
      setSelectedProperties([...selectedProperties, p]);
    } else if (!isChecked) {
      setSelectedProperties(
        selectedProperties.filter((existing) => existing !== p)
      );
    }
  };
  return html`
    <${Fragment}>
      <div className="properties-checkboxes">
        ${properties.map(
          (p) => html`
            <label for="people-container-${p}">
              <input
                id="people-container-${p}"
                type="checkbox"
                value=${p}
                onChange=${(e) => handleCheck(p, e.target.checked)}
              />
              ${p}
            </label>
          `
        )}
      </div>
      ${
        selectedProperties.length
          ? html`
              <${PeopleTable} properties=${selectedProperties} />
              <${PeopleGraph} properties=${selectedProperties} />
            `
          : html`<p>Select at least one property to see a visualization.</p>`
      }
    </Fragment>
  `;
}

export function PeopleGraph({ properties }) {
  const plotRef = useRef(null);
  const [history, setHistory] = useState({
    property_key: "",
    data: [],
  });
  const { currentTime, generation } = useAppState();
  const property_key = properties.join("");

  useEffect(() => {
    (async () => {
      const results = await tabulateProperties(properties);
      const newHistoryItems = results.map(([propMap, value]) => ({
        time: currentTime,
        value,
        series: properties.map((prop) => `${prop}:${propMap[prop]}`).join("|"),
      }));
      setHistory((h) =>
        h.property_key === property_key
          ? { ...h, data: [...h.data, ...newHistoryItems] }
          : { property_key, data: newHistoryItems }
      );
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

  return html`
    <div className="people-graph">
      <h3>Timeseries</h3>
      <div className="chart-container" ref=${plotRef}></div>
    </div>
  `;
}

export function PeopleTable({ properties }) {
  const { generation, currentTime } = useAppState();
  const [tabulated, setTabulated] = useState([]);
  useEffect(() => {
    (async () => {
      const result = await tabulateProperties(properties);
      const tableRows = result.map(([propMap, count]) => ({
        key: properties.map((prop) => propMap[prop]).join("|"),
        data: [...properties.map((prop) => propMap[prop]), count],
      }));
      setTabulated(tableRows);
    })();
  }, [generation, properties.join("|")]);

  const headerColumns = [...properties, "Count"];

  return html`
    <div>
      <h3>People Status at t = ${currentTime.toFixed(1)}</h3>
      <table className="table">
        <thead>
          <tr>
            ${headerColumns.map((prop) => html`<th key=${prop}>${prop}</th>`)}
          </tr>
        </thead>
        <tbody>
          ${tabulated.map(
            (r) => html`
              <tr key=${r.key}>
                ${r.data.map((c, i) => html`<td key=${i}>${c}</td>`)}
              </tr>
            `
          )}
        </tbody>
      </table>
    </div>
  `;
}
