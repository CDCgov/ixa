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
      Global: { Get: { property: setting } },
    });

    return response.Value;
  }

  async function nextTime(t) {
    await makeApiCall("next", {
      Next: {
        next_time: t,
      },
    });
  }

  const baseUrl = getBaseUrl();

  return {
    getPopulation,
    getTime,
    getGlobalSettingsList,
    getGlobalSettingValue,
    nextTime,
  };
}

let api = null;

export default async function getApi() {
  if (!api) {
    api = await new Api();
  }
  return api;
}
