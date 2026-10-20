// SPDX-License-Identifier: MIT
import { createRoot } from "react-dom/client";
import { installInstrumentation } from "./instrument.js";
import App from "./App.jsx";

installInstrumentation();

const root = createRoot(document.getElementById("root"));
root.render(<App />);
