import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/inter";
import "@fontsource-variable/archivo/wdth.css";
import "virtual:nuvora-tokens.css";
import "../styles/global.css";
import { desktop } from "../platform/desktop";
import { Connect } from "./Connect";

// Only the desktop apps have a connect page; a browser that lands here goes to the app.
if (!desktop) location.replace("/");
else {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <Connect bridge={desktop} />
    </StrictMode>,
  );
}
