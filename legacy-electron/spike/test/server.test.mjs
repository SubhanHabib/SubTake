import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { WebSocket } from "ws";
import { startService } from "../server.mjs";
import { initialProject } from "../core.mjs";
async function setup(t) {
	const folder = await mkdtemp(join(tmpdir(), "subtake-spike-test-"));
	const media = join(folder, "fixture.mp4");
	await writeFile(media, Buffer.from("0123456789"));
	const service = await startService({ media, work: join(folder, "output") });
	t.after(async () => {
		service.close();
		await rm(folder, { recursive: true, force: true });
	});
	return { ...service, media: await realpath(media) };
}
const headers = (s) => ({ "Content-Type": "application/json", Authorization: `Bearer ${s.token}` });
test("service rejects unauthenticated, cross-origin, and outside-media project writes", async (t) => {
	const s = await setup(t);
	let r = await fetch(`${s.origin}/api/open`, { method: "POST" });
	assert.equal(r.status, 401);
	r = await fetch(`${s.origin}/api/open`, {
		method: "POST",
		headers: { ...headers(s), Origin: "https://example.com" },
	});
	assert.equal(r.status, 403);
	r = await fetch(`${s.origin}/api/save`, {
		method: "POST",
		headers: headers(s),
		body: JSON.stringify(initialProject("/outside.mp4")),
	});
	assert.equal(r.status, 400);
});
test("media ranges and project round-trip preserve data", async (t) => {
	const s = await setup(t);
	const r = await fetch(`${s.origin}/media?token=${s.token}`, {
		headers: { Range: "bytes=2-5" },
	});
	assert.equal(r.status, 206);
	assert.equal(r.headers.get("content-range"), "bytes 2-5/10");
	assert.equal(await r.text(), "2345");
	const invalid = await fetch(`${s.origin}/media?token=${s.token}`, {
		headers: { Range: "bytes=100-200" },
	});
	assert.equal(invalid.status, 416);
	const project = initialProject(s.media);
	project.editor.custom = "preserved";
	const saved = await fetch(`${s.origin}/api/save`, {
		method: "POST",
		headers: headers(s),
		body: JSON.stringify(project),
	});
	assert.equal(saved.status, 200);
	const loaded = await fetch(`${s.origin}/api/load`, { method: "POST", headers: headers(s) });
	assert.deepEqual(await loaded.json(), project);
});
test("external command transport correlates replies and rejects renderer disconnect", async (t) => {
	const s = await setup(t);
	const ws = new WebSocket(`${s.origin.replace("http:", "ws:")}/ws?token=${s.token}`, {
		origin: s.origin,
	});
	await new Promise((resolve, reject) => {
		ws.once("open", resolve);
		ws.once("error", reject);
	});
	ws.once("message", (raw) => {
		const request = JSON.parse(raw);
		assert.equal(request.name, "set_playhead");
		ws.send(JSON.stringify({ id: request.id, result: { seconds: 2 } }));
	});
	assert.deepEqual(await s.command("set_playhead", { seconds: 2 }), { seconds: 2 });
	ws.once("message", () => ws.close());
	await assert.rejects(s.command("render_preview"), /disconnected/);
});
