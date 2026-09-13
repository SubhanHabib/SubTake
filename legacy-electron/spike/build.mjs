import { build } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL(".", import.meta.url));
await build({
	configFile: false,
	root,
	plugins: [react()],
	publicDir: "../public",
	resolve: { alias: { "@": resolve(root, "../src") } },
	build: { outDir: "dist", target: "es2022", sourcemap: true },
});
