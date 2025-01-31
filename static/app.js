import React from "https://esm.sh/react@19/?dev";
import ReactDOMClient from "https://esm.sh/react-dom@19/client?dev";
import htm from "https://esm.sh/htm@3?dev";
import getApi from "./api.js";

import { useEffect, useState } from "https://esm.sh/react@19/?dev";

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

  return html`
    <div><${Time} app=${app} /></div>
    <div><${Population} app=${app} /></div>
    <div><${GlobalSettings} app=${app} /></div>
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
