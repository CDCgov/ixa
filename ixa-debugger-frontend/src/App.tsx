import logo from "./assets/ixa.png";
import "./App.css";
import { useSimulation } from "./useGeneration";
import { useEffect, useState } from "react";
import {
    getGlobalSettingsList,
    getPeoplePropertiesList,
    getPopulation,
} from "./api";
import { PeopleGraph } from "./PeopleGraph";

function Population() {
    const { generation } = useSimulation();
    const [population, setPopulation] = useState(0);

    useEffect(() => {
        getPopulation().then((p) => setPopulation(p));
    }, [generation]);

    return (
        <dl>
            <dt>Population:</dt> <dd>{population}</dd>
        </dl>
    );
}

function GlobalSettings() {
    const [settings, setSettings] = useState<string[]>([]);

    useEffect(() => {
        getGlobalSettingsList().then((s) => setSettings(s));
    }, []);

    return (
        <div className="settings">
            <h2>Global Settings</h2>
            <ul>
                {settings.map((setting) => (
                    <li key={setting}>{setting}</li>
                ))}
            </ul>
        </div>
    );
}

function PersonProperties() {
    const [properties, setProperties] = useState<string[]>([]);

    useEffect(() => {
        getPeoplePropertiesList().then((p) => setProperties(p));
    }, []);

    return (
        <div className="settings">
            <h2>Person Properties</h2>
            <ul>
                {properties.map((property) => (
                    <li key={property}>{property}</li>
                ))}
            </ul>
        </div>
    );
}

function App() {
    const { currentTime, goNext } = useSimulation();

    async function doApiCall() {
        goNext();
    }

    return (
        <>
            <header>
                <div id="logo">
                    <a href="https://github.com/cdcgov/ixa" target="_blank">
                        <img src={logo} className="logo" alt="Vite logo" />
                    </a>
                </div>
                <div>
                    <h1>Ixa Debugger</h1>
                </div>
                <div>
                    Current time: <strong>{currentTime.toFixed(1)}</strong>
                </div>
                <div>
                    <button onClick={() => doApiCall()}>Advance time</button>
                </div>
            </header>
            <div className="body-wrapper">
                <aside>
                    <div className="panel">
                        <Population />
                    </div>
                    <div className="panel">
                        <GlobalSettings />
                    </div>
                    <div className="panel">
                        <PersonProperties />
                    </div>
                </aside>
                <main>
                    <PeopleGraph properties={["InfectionStatus"]} />
                </main>
            </div>
        </>
    );
}

export default App;
