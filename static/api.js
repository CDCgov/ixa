let prefix;
function getPrefix() {
  let url = new URL(window.location);
  const prefix = url.pathname.split("/")[1];
  if (prefix?.length != 36) {
    throw Error("Malformed URL; expected to contain ?token={TOKEN}");
  }
  return prefix;
}

export async function makeApiCall(endpoint, body) {
  prefix = prefix || getPrefix();
  const url = `/${prefix}/cmd/${endpoint}`;
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

export async function getGlobalSettingValue(setting) {
  const response = await makeApiCall("global", {
    Global: { Get: { property: setting } },
  });
  return response.Value;
}

export async function nextTime(t) {
  await makeApiCall("next", {
    Next: {
      next_time: t,
    },
  });
}

export async function tabulateProperties(props) {
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
