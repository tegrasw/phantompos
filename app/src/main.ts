import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { initLocale } from "./lib/i18n/locale.svelte";

// Before mounting, so the first render is already in the right language.
initLocale();

const target = document.getElementById("app");
if (!target) throw new Error("Missing #app mount element");

const app = mount(App, { target });

export default app;
