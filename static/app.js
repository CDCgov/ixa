import React from "https://esm.sh/react@19/?dev";
import ReactDOMClient from "https://esm.sh/react-dom@19/client?dev";
import htm from "https://esm.sh/htm@3?dev";
import getApi from "./api.js";
import * as Plot from "https://cdn.jsdelivr.net/npm/@observablehq/plot@0.6/+esm";
import { useEffect, useState, useRef } from "https://esm.sh/react@19/?dev";

let currentTime;

// For tagged string templating
const html = htm.bind(React.createElement);

function makeAppState() {
  let [generation, setGeneration] = useState(0);

  function update() {
    console.log("Triggering app update");
    setGeneration(generation + 1);
  }

  return {
    update,
  };
}

function App() {
  let app = makeAppState();
  let properties = ["InfectionStatus"];

  return html`
    <div><${Time} app=${app} /></div>
    <div><${Population} app=${app} /></div>
    <div><${GlobalSettings} app=${app} /></div>
    <div><${PeoplePropertiesList} app=${app} /></div>
    <div><${TabulatedPeople} app=${app} properties=${properties} /></div>
    <div><${PeopleGraph} app=${app} properties=${properties} /></div>
    <div><${NextButton} app=${app} /></div>
  `;
}

function Time({ app }) {
  let [time, setTime] = useState(0);
  useEffect(() => {
    (async () => {
      let api = await getApi();

      const now = await api.getTime();
      setTime(now);
      currentTime = now;
    })();
  }, [app]);

  return html` <div><b>Simulation Time: </b> ${time}</div> `;
}

function Population({ app }) {
  let [population, setPopulation] = useState(0);

  useEffect(() => {
    (async () => {
      let api = await getApi();

      const pop = await api.getPopulation();
      setPopulation(pop);
    })();
  }, [app]);

  return html` <div><b>Population: </b> ${population}</div> `;
}

function GlobalSettings({ app }) {
  let [globals, setGlobals] = useState([]);

  useEffect(() => {
    (async () => {
      let api = await getApi();

      let globalProperties = await api.getGlobalSettingsList();
      let listValues = [];
      for (let propertyName of globalProperties) {
        let value = await api.getGlobalSettingValue(propertyName);

        listValues.push(
          html`<li key="${propertyName}">${propertyName} = ${value}</li>`,
        );
      }

      setGlobals(listValues);
    })();
  }, [app]);

  return html`<div>
    <b>Global Properties</b>
    <ul>
      ${globals}
    </ul>
  </div>`;
}

function TabulatedPeople({ app, properties }) {
  let [tabulated, setTabulated] = useState([]);

  useEffect(() => {
    (async () => {
      let api = await getApi();

      let result = await api.tabulateProperties(properties);
      let tableRows = [];
      for (let row of result) {
        let columns = properties.map((prop) => row[0][prop]);
        columns.push(row[1]);
        let tableRow = columns.map((c) => html`<td>${c}</td>`);
        tableRows.push(
          html`<tr>
            ${tableRow}
          </tr>`,
        );
      }
      console.log(tableRows);
      setTabulated(tableRows);
    })();
  }, [app]);

  let headerColumns = [...properties, "Count"].map(
    (prop) => html`<td>${prop}</td>`,
  );

  return html`<div>
    <b>People Status</b>
    <table>
      <thead>
        <tr>
          ${headerColumns}
        </tr>
      </thead>
      <tbody>
        ${tabulated}
      </tbody>
    </table>
  </div>`;
}

function PeoplePropertiesList({ app }) {
  let [propertyList, setPropertyList] = useState();

  useEffect(() => {
    (async () => {
      let api = await getApi();

      let peopleProperties = await api.getPeoplePropertiesList();
      let listValues = [];
      for (let propertyName of peopleProperties) {
        listValues.push(html`<li key="${propertyName}">${propertyName}</li>`);
      }

      setPropertyList(listValues);
    })();
  }, [app]);

  return html`<div>
    <b>People Properties</b>
    <ul>
      ${propertyList}
    </ul>
  </div>`;
}

function PeopleGraph({ app, properties }) {
  const plotRef = useRef();
  let history = useRef([]);

  useEffect(() => {
    (async () => {
      let api = await getApi();

      let results = await api.tabulateProperties(properties);
      for (let result of results) {
        let label = [];
        for (let prop of properties) {
          label.push(`${prop}:${result[0][prop]}`);
        }
        history.current.push({
          time: currentTime,
          value: result[1],
          series: label.join("|"),
        });
      }

      console.log(history.current);
      const plot = Plot.plot({
        color: { legend: true },

        marks: [
          Plot.line(history.current, {
            x: "time",
            y: "value",
            stroke: "series",
          }),
        ],
      });
      plotRef.current?.replaceChildren(plot);
    })();
  }, [app, properties]);

  return html`<div className="people-graph">
    <b>Time Series</b>
    <div ref=${plotRef}></div>
  </div>`;
}
function NextButton({ app }) {
  async function goToNextTime(api, next) {
    let now = currentTime;
    await api.nextTime(next);

    // Busy wait on the API until time changes.
    while (now === currentTime) {
      now = await api.getTime();
    }

    app.update();
    currentTime = now;
  }

  function handleClick() {
    (async () => {
      let api = await getApi();
      goToNextTime(api, currentTime + 1.0);
    })();
  }

  return html`<button onClick="${handleClick}">Next</button>`;
}
ReactDOMClient.createRoot(document.getElementById("root")).render(
  React.createElement(App, {}, null),
);
