import { useEffect, useRef, useState, Fragment } from "react";
import { html } from "htm/react";
import { useAppState } from "./app-state.js";
import * as Plot from "@observablehq/plot";

const CHART_TYPES = [
  { label: "Table", value: "table", component: PeopleTable },
  { label: "Timeseries", value: "timeseries", component: PeopleTimeseries },
];

function chartConfig({ chartType, selectedProperties }) {
  return { chartType, selectedProperties };
}

function getChartType(chartType) {
  let c = CHART_TYPES.find((c) => c.value === chartType);
  if (!c) {
    console.error(`Unknown chart type: ${chartType}`);
  }
  return c?.component;
}

function useChartState() {
  const [charts, setCharts] = useState([]);

  const addChart = (chartType, selectedProperties) => {
    setCharts([...charts, chartConfig({ chartType, selectedProperties })]);
  };
  const removeChart = (index) =>
    setCharts(charts.filter((_, i) => i !== index));
  return [charts, addChart, removeChart];
}

export function PeopleChartsContainer() {
  const { generation, api } = useAppState();
  const [properties, setProperties] = useState([]);
  const [selectedChartType, setSelectedChartType] = useState("table");
  const [selectedProperties, setSelectedProperties] = useState([]);
  const [charts, addChart, removeChart] = useChartState();

  useEffect(() => {
    api.getPeoplePropertiesList().then((p) => setProperties(p));
  }, [generation]);

  function handleCheck(p, isChecked) {
    if (isChecked && !selectedProperties.includes(p)) {
      setSelectedProperties([...selectedProperties, p]);
    } else if (!isChecked) {
      setSelectedProperties(
        selectedProperties.filter((existing) => existing !== p)
      );
    }
  }

  function onSubmit(e) {
    e.preventDefault();
    // TODO<ryl8@cdc.gov>: This should produce an error message
    if (selectedProperties.length === 0) {
      return;
    }
    addChart(selectedChartType, selectedProperties);
  }

  return html`
  <${Fragment}>
    <form className="create-chart-form" onSubmit=${onSubmit}>
      <div className="properties-checkboxes">
      ${properties.map(
        (p) => html`
          <label key=${p} htmlFor="people-container-${p}">
            <input
              id="people-container-${p}"
              type="checkbox"
              value=${p}
              onChange=${(e) => handleCheck(p, e.target.checked)}
            />
            ${p} </label
          >${" "}
        `
      )}
      </div>
      <div>
        <select
          value=${selectedChartType}
          onChange=${(e) => setSelectedChartType(e.target.value)}
        >
          ${CHART_TYPES.map(
            (c) => html`
              <option key=${c.value} value=${c.value}>${c.label}</option>
            `
          )}</select
        >${" "}
        <button title=${
          selectedProperties.length > 0
            ? ""
            : "Select at least one property to add a chart."
        } disabled=${selectedProperties.length === 0}>Add chart</button>
      </div>
    </form>
    ${
      charts.length
        ? charts.map(
            ({ chartType, selectedProperties }, i) => html`
              <div key=${i} className="chart-wrapper">
                <${getChartType(chartType)}
                  properties=${selectedProperties}
                  removeButton=${html`<a
                    href="#"
                    className="remove-button"
                    onClick=${(e) => {
                      e.preventDefault();
                      removeChart(i);
                    }}
                  >
                    [ Remove ]
                  </button>`}
                />
              </div>
            `
          )
        : html`<p>Select at least one property to add a chart.</p>`
    }
  </${Fragment}>`;
}

export function PeopleTimeseries({ properties, removeButton }) {
  const plotRef = useRef(null);
  const [history, setHistory] = useState({
    property_key: "",
    data: [],
  });
  const { currentTime, generation, api } = useAppState();
  const property_key = properties.join("");

  useEffect(() => {
    (async () => {
      const results = await api.tabulateProperties(properties);
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
      <h3>Timeseries ${removeButton}</h3>
      <div className="chart-container" ref=${plotRef}></div>
    </div>
  `;
}

export function PeopleTable({ properties, removeButton }) {
  const { generation, currentTime, api } = useAppState();
  const [tabulated, setTabulated] = useState([]);
  useEffect(() => {
    (async () => {
      const result = await api.tabulateProperties(properties);
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
      <h3>People Status at t = ${currentTime.toFixed(1)} ${removeButton}</h3>
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
