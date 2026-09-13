import http from "node:http";
import { readFile, writeFile, mkdir, stat, realpath, copyFile, rename } from "node:fs/promises";
import { createReadStream } from "node:fs";
import { resolve, extname, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { randomBytes, randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { WebSocketServer } from "ws";
import { validateProject } from "./core.mjs";
const root = fileURLToPath(new URL(".", import.meta.url));
export async function startService({ media, work }) {
	await mkdir(work, { recursive: true });
	media = await realpath(media);
	const token = randomBytes(32).toString("hex");
	const reports = [];
	const pending = new Map();
	let client;
	let origin;
	const command = (name, args = {}) =>
		new Promise((resolve, reject) => {
			if (!client || client.readyState !== 1)
				return reject(Error("Editor command bus is not connected"));
			const id = randomUUID();
			const timer = setTimeout(() => {
				pending.delete(id);
				reject(Error("Command timed out"));
			}, 120000);
			pending.set(id, { resolve, reject, timer });
			client.send(JSON.stringify({ id, name, args }));
		});
	const json = (response, code, value) => {
		response.writeHead(code, {
			"Content-Type": "application/json",
			"Cache-Control": "no-store",
		});
		response.end(JSON.stringify(value));
	};
	const server = http.createServer(async (req, res) => {
		try {
			if (req.headers.host !== new URL(origin).host) throw Error("Invalid host");
			const url = new URL(req.url, origin);
			const authorized =
				req.headers.authorization === `Bearer ${token}` ||
				url.searchParams.get("token") === token;
			if ((url.pathname.startsWith("/api/") || url.pathname === "/media") && !authorized)
				return json(res, 401, { error: "Unauthorized" });
			if (req.headers.origin && req.headers.origin !== origin)
				return json(res, 403, { error: "Invalid origin" });
			if (url.pathname === "/media") {
				const size = (await stat(media)).size;
				let start = 0,
					end = size - 1;
				const range = req.headers.range;
				if (range) {
					const match = /^bytes=(\d+)-(\d*)$/.exec(range);
					if (!match)
						return res.writeHead(416, { "Content-Range": `bytes */${size}` }).end();
					start = Number(match[1]);
					end = match[2] ? Math.min(Number(match[2]), end) : end;
					if (start > end || start >= size)
						return res.writeHead(416, { "Content-Range": `bytes */${size}` }).end();
				}
				res.writeHead(range ? 206 : 200, {
					"Content-Type": extname(media) === ".webm" ? "video/webm" : "video/mp4",
					"Accept-Ranges": "bytes",
					"Content-Length": end - start + 1,
					...(range ? { "Content-Range": `bytes ${start}-${end}/${size}` } : {}),
				});
				createReadStream(media, { start, end }).pipe(res);
				return;
			}
			if (url.pathname.startsWith("/api/")) {
				if (req.method !== "POST") return json(res, 405, { error: "POST required" });
				let bytes = 0;
				const chunks = [];
				for await (const chunk of req) {
					bytes += chunk.length;
					if (bytes > 64 * 1024 * 1024) throw Error("Request too large");
					chunks.push(chunk);
				}
				const body = JSON.parse(Buffer.concat(chunks).toString() || "{}");
				let result;
				switch (url.pathname) {
					case "/api/open":
						result = { path: media, url: `/media?token=${token}` };
						break;
					case "/api/save": {
						const project = validateProject(body);
						if (project.videoPath !== media)
							throw Error("Media is outside fixture scope");
						const temporary = resolve(work, `project-${randomUUID()}.tmp`);
						await writeFile(temporary, JSON.stringify(project, null, 2));
						await rename(temporary, resolve(work, "project.json"));
						result = { saved: true };
						break;
					}
					case "/api/load":
						result = validateProject(
							JSON.parse(await readFile(resolve(work, "project.json"), "utf8")),
						);
						break;
					case "/api/report":
						reports.push(body);
						await writeFile(
							resolve(work, "renderer.json"),
							JSON.stringify(reports, null, 2),
						);
						result = { ok: true };
						break;
					case "/api/command":
						result = await command(body.name, body.args);
						break;
					case "/api/preview": {
						if (
							typeof body.image !== "string" ||
							!body.image.startsWith("data:image/png;base64,")
						)
							throw Error("PNG required");
						await writeFile(
							resolve(work, "preview.png"),
							Buffer.from(body.image.split(",")[1], "base64"),
						);
						result = { path: resolve(work, "preview.png") };
						break;
					}
					case "/api/hyperframes": {
						if (typeof body.html !== "string" || body.html.length > 20000)
							throw Error("Invalid composition");
						const folder = resolve(work, "hyperframes");
						await mkdir(folder, { recursive: true });
						await writeFile(resolve(folder, "index.html"), body.html);
						await copyFile(
							resolve(root, "node_modules/gsap/dist/gsap.min.js"),
							resolve(folder, "gsap.min.js"),
						);
						result = { path: folder };
						break;
					}
					case "/api/export": {
						if (
							!Array.isArray(body.frames) ||
							body.frames.length !== 30 ||
							body.frames.some(
								(f) =>
									typeof f !== "string" ||
									!f.startsWith("data:image/png;base64,"),
							)
						)
							throw Error("Expected exactly 30 PNG frames");
						const folder = resolve(work, `export-${randomUUID()}`);
						await mkdir(folder);
						for (let i = 0; i < 30; i++)
							await writeFile(
								resolve(folder, `${String(i).padStart(3, "0")}.png`),
								Buffer.from(body.frames[i].split(",")[1], "base64"),
							);
						const output = resolve(folder, "preview.mp4");
						await execute("ffmpeg", [
							"-hide_banner",
							"-loglevel",
							"error",
							"-framerate",
							"15",
							"-i",
							resolve(folder, "%03d.png"),
							"-c:v",
							"libx264",
							"-preset",
							"fast",
							"-crf",
							"18",
							"-pix_fmt",
							"yuv420p",
							"-movflags",
							"+faststart",
							output,
						]);
						result = {
							path: output,
							bytes: (await stat(output)).size,
							frames: 30,
							fps: 15,
							audio: false,
							backend: "Pixi PNG + ffmpeg libx264",
						};
						break;
					}
					default:
						return json(res, 404, { error: "Unknown operation" });
				}
				return json(res, 200, result);
			}
			const base = resolve(root, "dist");
			let file = resolve(
				base,
				"." + decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname),
			);
			file = await realpath(file);
			if (!file.startsWith(base + sep)) throw Error("Outside static root");
			const mime =
				{
					".js": "text/javascript",
					".css": "text/css",
					".html": "text/html",
					".svg": "image/svg+xml",
					".png": "image/png",
					".jpg": "image/jpeg",
					".webp": "image/webp",
					".woff2": "font/woff2",
				}[extname(file)] ?? "application/octet-stream";
			res.writeHead(200, {
				"Content-Type": mime,
				"Referrer-Policy": "no-referrer",
				"Content-Security-Policy":
					"default-src 'self'; script-src 'self' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' data: blob:; connect-src 'self' ipc: http://ipc.localhost data: blob: ws://127.0.0.1:*; worker-src 'self' blob:",
			});
			res.end(await readFile(file));
		} catch (error) {
			if (!res.headersSent) json(res, 400, { error: String(error) });
			else res.destroy();
		}
	});
	const wss = new WebSocketServer({ noServer: true, maxPayload: 16 * 1024 * 1024 });
	server.on("upgrade", (req, socket, head) => {
		const url = new URL(req.url, origin);
		if (
			url.pathname !== "/ws" ||
			url.searchParams.get("token") !== token ||
			req.headers.origin !== origin
		) {
			socket.destroy();
			return;
		}
		wss.handleUpgrade(req, socket, head, (ws) => wss.emit("connection", ws));
	});
	wss.on("connection", (ws) => {
		if (client) {
			ws.close();
			return;
		}
		client = ws;
		ws.on("message", (raw) => {
			try {
				const reply = JSON.parse(raw);
				const item = pending.get(reply.id);
				if (!item) return;
				clearTimeout(item.timer);
				pending.delete(reply.id);
				reply.error ? item.reject(Error(reply.error)) : item.resolve(reply.result);
			} catch {
				ws.close();
			}
		});
		ws.on("close", () => {
			client = undefined;
			for (const item of pending.values()) {
				clearTimeout(item.timer);
				item.reject(Error("Editor disconnected"));
			}
			pending.clear();
		});
	});
	await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
	origin = `http://127.0.0.1:${server.address().port}`;
	return {
		origin,
		token,
		reports,
		command,
		close: () => {
			client?.close();
			wss.close();
			server.close();
		},
	};
}
export function execute(cmd, args) {
	return new Promise((resolve, reject) => {
		const p = spawn(cmd, args, { stdio: ["ignore", "ignore", "pipe"] });
		let error = "";
		p.stderr.on("data", (b) => {
			error += b;
		});
		p.on("error", reject);
		p.on("exit", (code) => (code === 0 ? resolve() : reject(Error(`${cmd}: ${error}`))));
	});
}
