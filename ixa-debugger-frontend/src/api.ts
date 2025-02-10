export async function makeApiCall<T>(endpoint: string, body: T) {
    const prefix = import.meta.env.VITE_API_PREFIX;
    if (!prefix) {
        throw new Error(
            "Expected VITE_API_PREFIX to be set in the server environment"
        );
    }
    const url = `/api/${prefix}/cmd/${endpoint}`;
    const res = await fetch(url, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
    });
    if (!res.ok) {
        throw new Error(`Failed to fetch ${url}`);
    }
    return res.json();
}

export async function getPopulation() {
    const response = await makeApiCall("population", {});
    return response.population;
}

export async function getTime() {
    const response = await makeApiCall("time", {});
    return response.time;
}

export async function getGlobalSettingsList() {
    const response = await makeApiCall("global", {
        Global: "List",
    });

    return response.List;
}

export async function getGlobalSettingValue(setting: string) {
    const response = await makeApiCall("global", {
        Global: { Get: { property: setting } },
    });
    return response.Value;
}

export async function nextTime(t: number) {
    await makeApiCall("next", {
        Next: {
            next_time: t,
        },
    });
}

export async function tabulateProperties(props: string[]) {
    const response = await makeApiCall("people", {
        People: {
            Tabulate: {
                properties: props,
            },
        },
    });
    return response.Tabulated;
}

export async function getPeoplePropertiesList() {
    const response = await makeApiCall("people", {
        People: "Properties",
    });

    return response.PropertyNames;
}
