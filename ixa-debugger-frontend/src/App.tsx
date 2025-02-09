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
        <div className="population">
            <h2>Population: {population}</h2>
        </div>
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
    const { generation, currentTime, goNext } = useSimulation();

    async function doApiCall() {
        goNext();
    }

    return (
        <>
            <div>
                <a href="https://github.com/cdcgov/ixa" target="_blank">
                    <img src={logo} className="logo" alt="Vite logo" />
                </a>
            </div>
            <h1>Time: {currentTime}</h1>
            <Population />
            <GlobalSettings />
            <PersonProperties />
            <PeopleGraph properties={["InfectionStatus"]} />
            <div className="card">
                <button onClick={() => doApiCall()}>
                    count is {generation}
                </button>
            </div>
            <p className="read-the-docs">Click on the Ixa logo to learn more</p>
        </>
    );
}

export default App;
