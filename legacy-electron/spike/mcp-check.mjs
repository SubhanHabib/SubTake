import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { resolve } from "node:path";
import { writeFile } from "node:fs/promises";
import { percentile } from "./core.mjs";
const config = resolve(process.argv[2]);
const client = new Client({ name: "spike-validation", version: "1.0" });
const transport = new StdioClientTransport({
	command: process.execPath,
	args: [resolve(import.meta.dirname, "mcp.mjs"), config],
});
await client.connect(transport);
try {
	const listed = await client.listTools();
	const samples = [];
	for (const [name, args] of [
		["open_project", {}],
		["add_zoom_region", { startMs: 2500, endMs: 4500, depth: 2 }],
		["export_video", {}],
		["set_caption", { text: "MCP command proof", startMs: 0, endMs: 2000 }],
	]) {
		const result = await client.callTool({ name, arguments: args });
		if (result.isError) throw Error(JSON.stringify(result));
	}

	for (let i = 0; i < 10; i++) {
		const start = performance.now();
		const result = await client.callTool({
			name: "set_playhead",
			arguments: { seconds: i / 3 },
		});
		if (result.isError) throw Error(JSON.stringify(result));
		samples.push(performance.now() - start);
	}
	const result = await client.callTool({ name: "render_preview", arguments: { seconds: 1 } });
	if (result.isError || result.content[0].type !== "image") throw Error("Preview MCP failed");
	await writeFile(
		resolve(config, "../mcp-preview.png"),
		Buffer.from(result.content[0].data, "base64"),
	);
	const report = {
		tools: listed.tools.map((t) => t.name),
		samplesMs: samples,
		p95Ms: percentile(samples),
		preview: true,
	};
	await writeFile(resolve(config, "../mcp.json"), JSON.stringify(report, null, 2));
	console.log(JSON.stringify(report));
} finally {
	await client.close();
}
