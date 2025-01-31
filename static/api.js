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

  async function getGlobalSettings() {
    let response = await makeApiCall("global", {
      Global: "List",
    });

    return response.List;
  }

  const baseUrl = getBaseUrl();

  return {
    getPopulation,
  };
}

let api = null;

export default async function getApi() {
  if (!api) {
    api = await new Api();
  }
  return api;
}
