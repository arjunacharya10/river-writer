import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";
import solid from "vite-plugin-solid";
export default defineConfig({
  root: fileURLToPath(new URL("./", import.meta.url)),
  plugins: [solid()],
  server: {
    port: 1421,
    strictPort: true,
    fs: { allow: [fileURLToPath(new URL("../../", import.meta.url))] },
    watch: { ignored: ["**/target/**"] },
  },
  clearScreen: false,
});
