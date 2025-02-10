import React, { Fragment, useEffect, useState } from "react";
import { html } from "htm/react";
import {
  getGlobalSettingsList,
  getGlobalSettingValue,
  getPeoplePropertiesList,
  getPopulation,
} from "./api.js";
import { useAppState } from "./useAppState.js";
import { PeopleChartsContainer } from "./PeopleCharts.js";

function Population() {
  const { generation } = useAppState();
  const [population, setPopulation] = useState(0);

  useEffect(() => {
    getPopulation().then((p) => setPopulation(p));
  }, [generation]);

  return html`
    <dl>
      <dt>Population:</dt>
      <dd>${population}</dd>
    </dl>
  `;
}

function GlobalSettings() {
  const { generation } = useAppState();
  const [settings, setSettings] = useState({});

  useEffect(() => {
    (async () => {
      const newSettings = {};
      const globalProperties = await getGlobalSettingsList();
      for (const propertyName of globalProperties) {
        const value = await getGlobalSettingValue(propertyName);
        newSettings[propertyName] = value;
      }
      setSettings(newSettings);
    })();
  }, [generation]);

  return html`
    <div className="settings">
      <h2>Global Settings</h2>
      <ul>
        ${Object.entries(settings).map(
          ([propertyName, value]) => html` <li key=${propertyName}>
            <span className="key">${propertyName}</span>: ${value}
          </li>`
        )}
      </ul>
    </div>
  `;
}

function PersonProperties() {
  const { generation } = useAppState();
  const [properties, setProperties] = useState([]);

  useEffect(() => {
    getPeoplePropertiesList().then((p) => setProperties(p));
  }, [generation]);

  return html`
    <div className="settings">
      <h2>Person Properties</h2>
      <ul>
        ${properties.map(
          (property) => html` <li key=${property}>
            <span className="key">${property}</span>
          </li>`
        )}
      </ul>
    </div>
  `;
}

function App() {
  const { currentTime, goNext } = useAppState();

  async function doApiCall() {
    goNext();
  }

  return html`
      <${Fragment}>
        <header>
          <div id="logo">
            <a href="https://github.com/cdcgov/ixa" target="_blank">
              <img src="ixa.png" className="logo" alt="Ixa logo" />
            </a>
          </div>
          <div>
            <h1>Ixa Debugger</h1>
          </div>
          <div>
            Current time: <strong>${currentTime.toFixed(1)}</strong>
          </div>
          <div>
            <button onClick=${doApiCall}>Advance time</button>
          </div>
        </header>
        <div className="body-wrapper">
          <aside>
            <div className="panel">${html`<${Population} />`}</div>
            <div className="panel">${html`<${GlobalSettings} />`}</div>
            <div className="panel">${html`<${PersonProperties} />`}</div>
          </aside>
          <main>${html`<${PeopleChartsContainer} />`}</main>
        </div>
      </>
    `;
}



export default App;
