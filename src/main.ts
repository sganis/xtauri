import 'bootstrap/dist/css/bootstrap.min.css';
import 'bootstrap-icons/font/bootstrap-icons.css'
import "./styles.css";
import App from "./views/App.svelte";

const app = new App({
  target: document.getElementById("app"),
});

export default app;
