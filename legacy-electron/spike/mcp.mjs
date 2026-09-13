import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { ListToolsRequestSchema, CallToolRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { readFile } from "node:fs/promises";
const config = JSON.parse(await readFile(process.argv[2], "utf8"));
const server = new Server(
	{ name: "subtake-spike", version: "0.1.0" },
	{ capabilities: { tools: {} } },
);
const schemas = {
	open_project: { type: "object", properties: {} },
	add_zoom_region: {
		type: "object",
		properties: {
			startMs: { type: "number" },
			endMs: { type: "number" },
			depth: { type: "integer", minimum: 1, maximum: 6 },
		},
		required: ["startMs", "endMs"],
	},
	set_caption: {
		type: "object",
		properties: {
			text: { type: "string", maxLength: 200 },
			startMs: { type: "number" },
			endMs: { type: "number" },
		},
		required: ["text"],
	},
	set_playhead: {
		type: "object",
		properties: { seconds: { type: "number", minimum: 0 } },
		required: ["seconds"],
	},
	render_preview: { type: "object", properties: { seconds: { type: "number", minimum: 0 } } },
	export_video: { type: "object", properties: {} },
};
server.setRequestHandler(ListToolsRequestSchema, async () => ({
	tools: Object.entries(schemas).map(([name, inputSchema]) => ({
		name,
		description:
			name === "export_video"
				? "Export 2 seconds of Pixi video/zoom only, silent; rejects non-supported layers"
				: name === "open_project"
					? "Open the saved fixture project"
					: `Run ${name} through the shared project command bus`,
		inputSchema,
	})),
}));
server.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
	try {
		if (!schemas[params.name]) throw Error("Unknown tool");
		const response = await fetch(`${config.origin}/api/command`, {
			method: "POST",
			headers: {
				"Content-Type": "application/json",
				Authorization: `Bearer ${config.token}`,
			},
			body: JSON.stringify({ name: params.name, args: params.arguments ?? {} }),
			signal: AbortSignal.timeout(125000),
		});
		const result = await response.json();
		if (!response.ok) throw Error(result.error);
		return {
			content: result.image
				? [{ type: "image", data: result.image.split(",")[1], mimeType: "image/png" }]
				: [{ type: "text", text: JSON.stringify(result) }],
		};
	} catch (e) {
		return { isError: true, content: [{ type: "text", text: String(e) }] };
	}
});
await server.connect(new StdioServerTransport());
