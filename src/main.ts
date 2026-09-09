// Bootstrap da aplicação: monta o componente raiz (App.vue)
// no elemento #app do index.html e carrega o CSS global (Tailwind).
import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

createApp(App).mount("#app");
