export let API_PREFIX = "";

export async function fetchApiPrefix() {
    const response = await fetch("/config.json");
    const data = await response.json();
    API_PREFIX = data.apiPrefix;
}

export async function apiCall<T>(endpoint: string, body: T) {
    if (!API_PREFIX) {
        await fetchApiPrefix();
    }
    const url = `/api/${API_PREFIX}/cmd${endpoint}`;
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
