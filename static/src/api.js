function Api() {
    function getBaseUrl() {
        let url = new URL(window.location);
        const prefix = url.pathname.split("/")[1];
        if (prefix.length != 36) {
            throw Error("Malformed URL");
        }
        url.pathname = `/${prefix}`;
        return url;
    }

    async function makeApiCall(cmd, body) {
        const url = new URL(baseUrl);
        url.pathname += `/cmd/${cmd}`;
        console.log(`makeApiCall ${url.toString()}`);

        let response = await fetch(url.toString(), {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
            },
            body: JSON.stringify(body),
        });

        if (!response.ok) {
            throw new Error((await response.json()).error);
        }

        return await response.json();
    }

    async function getPopulation() {
        let response = await makeApiCall("population", {});

        return response.population;
    }

    async function getTime() {
        let response = await makeApiCall("time", {});

        return response.time;
    }

    async function getGlobalSettingsList() {
        let response = await makeApiCall("global", {
            Global: "List",
        });

        return response.List;
    }

    async function getGlobalSettingValue(setting) {
        let response = await makeApiCall("global", {
            Global: {Get: {property: setting}},
        });

        return response.Value;
    }

    async function next() {
        await makeApiCall("next", {});
    }

    async function halt() {
        await makeApiCall("halt", {});
    }

    async function continueSimulation() {
        await makeApiCall("continue", {});
    }

    async function setBreakpoint(t) {
        await makeApiCall("breakpoint", {
            Breakpoint: {
                Set: {
                    time: t,
                    console: false
                }
            }
        });
    }

    async function listBreakpoints() {
        let response = await makeApiCall("breakpoint", {
            Breakpoint: "List"
        });

        return response.List;
    }

    async function deleteBreakpoint(breakpoint_id) {
        await makeApiCall("breakpoint", {
            Breakpoint: {
                Delete: {
                    id: breakpoint_id
                }
            }
        });
    }

    async function clearBreakpoints() {
        await makeApiCall("breakpoint", {
            Breakpoint: {
                Delete: {
                    all: true
                }
            }
        });
    }

    async function enableBreakpoints() {
        await makeApiCall("breakpoint", {
            Breakpoint: "Enable"
        });
    }

    async function disableBreakpoints() {
        await makeApiCall("breakpoint", {
            Breakpoint: "Disable"
        });
    }

    async function tabulateProperties(props) {
        let response = await makeApiCall("people", {
            People: {
                Tabulate: {
                    properties: props,
                },
            },
        });
        return response.Tabulated;
    }

    async function getPeoplePropertiesList() {
        let response = await makeApiCall("people", {
            People: "Properties",
        });

        return response.PropertyNames;
    }

    const baseUrl = getBaseUrl();

    return {
        getPopulation,
        getTime,
        getGlobalSettingsList,
        getGlobalSettingValue,
        next,
        halt,
        continueSimulation,
        listBreakpoints,
        setBreakpoint,
        deleteBreakpoint,
        clearBreakpoints,
        enableBreakpoints,
        disableBreakpoints,
        tabulateProperties,
        getPeoplePropertiesList,
    };
}

let api = null;

export default function getApi() {
    if (!api) {
        api = Api();
    }
    return api;
}
