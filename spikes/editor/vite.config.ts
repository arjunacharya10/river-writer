import {defineConfig} from "vite";
import {fileURLToPath} from "node:url";
import solid from "vite-plugin-solid";
export default defineConfig({
  plugins:[solid()],
  server:{port:1420,strictPort:true,fs:{allow:[
    fileURLToPath(new URL("./",import.meta.url)),
    fileURLToPath(new URL("../../fonts",import.meta.url)),
    fileURLToPath(new URL("../template.json",import.meta.url)),
  ]},watch:{ignored:["**/src-tauri/target/**"]}},
  clearScreen:false,
});
