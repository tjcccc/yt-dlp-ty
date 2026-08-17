import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/globals.css";

// Window vibrancy (tauri.conf.json `windowEffects`) is macOS-only, and the
// CSS that lets it show through clears the window background — which would
// leave a transparent, unreadable window anywhere the effect doesn't exist.
// Gate it on the platform rather than assuming, so Linux/Windows keep their
// solid fallback background.
if (navigator.userAgent.includes("Macintosh")) {
  document.documentElement.classList.add("has-window-vibrancy");
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
