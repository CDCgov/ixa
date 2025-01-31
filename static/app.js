import React from "https://esm.sh/react@19/?dev";
import ReactDOMClient from "https://esm.sh/react-dom@19/client?dev";
import htm from "https://esm.sh/htm@3?dev";
import getApi from "./api.js";

import { useEffect, useState } from "https://esm.sh/react@19/?dev";

// For tagged string templating
const html = htm.bind(React.createElement);

function App() {
  return html` <${MyPopulation} /> `;
}

function MyPopulation() {
  let [population, setPopulation] = useState(0);

  useEffect(() => {
    (async () => {
      let api = await getApi();

      const pop = await api.getPopulation();
      console.log(pop);
      setPopulation(pop);
    })();
  }, []);

  return html` <div><b>Population: </b> ${population}</div> `;
}

ReactDOMClient.createRoot(document.getElementById("root")).render(
  React.createElement(App, {}, null),
);
