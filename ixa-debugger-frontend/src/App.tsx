import logo from "./assets/ixa.png";
import "./App.css";
import { useSimulation } from "./useGeneration";
import { useEffect, useState } from "react";
import {
    getGlobalSettingsList,
    getGlobalSettingValue,
    getPeoplePropertiesList,
    getPopulation,
} from "./api";
import { PeopleChartsContainer } from "./PeopleCharts";

export type Serializable = string | number | boolean | null | undefined;

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

type GlobalSettingsData = Record<string, Serializable>;
function GlobalSettings() {
    const { generation } = useSimulation();
    const [settings, setSettings] = useState<GlobalSettingsData>({});

    useEffect(() => {
        (async () => {
            const newSettings: GlobalSettingsData = {};
            const globalProperties = await getGlobalSettingsList();
            for (const propertyName of globalProperties) {
                const value = await getGlobalSettingValue(propertyName);
                newSettings[propertyName] = value;
            }
            setSettings(newSettings);
        })();
    }, [generation]);

    console.log(settings);

    return (
        <div className="settings">
            <h2>Global Settings</h2>
            <ul>
                {Object.entries(settings).map(([propertyName, value]) => (
                    <li key={propertyName}>
                        <span className="key">{propertyName}</span>: {value}
                    </li>
                ))}
            </ul>
        </div>
    );
}

function PersonProperties() {
    const { generation } = useSimulation();
    const [properties, setProperties] = useState<string[]>([]);

    useEffect(() => {
        getPeoplePropertiesList().then((p) => setProperties(p));
    }, [generation]);

    return (
        <div className="settings">
            <h2>Person Properties</h2>
            <ul>
                {properties.map((property) => (
                    <li key={property}>
                        <span className="key">{property}</span>
                    </li>
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
                    <PeopleChartsContainer />
                </main>
            </div>
        </>
    );
}

export default App;
