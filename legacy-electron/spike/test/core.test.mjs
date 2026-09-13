import test from "node:test";
import assert from "node:assert/strict";
import { applyCommand, initialProject, toHyperFrames, percentile } from "../core.mjs";
const state = () => ({
	project: initialProject("/fixture.mp4"),
	playhead: 0,
	duration: 10,
	revision: 0,
});
test("commands preserve original state and unknown editor fields", () => {
	const original = state();
	original.project.editor.wallpaper = "custom";
	const next = applyCommand(original, "add_zoom_region", { startMs: 100, endMs: 900 });
	assert.equal(original.project.editor.zoomRegions.length, 0);
	assert.equal(next.project.editor.zoomRegions.length, 1);
	assert.equal(next.project.editor.wallpaper, "custom");
});
test("invalid temporal and zoom inputs are rejected", () => {
	for (const seconds of [-1, NaN, Infinity, 11, "2"])
		assert.throws(() => applyCommand(state(), "set_playhead", { seconds }));
	for (const args of [
		{ startMs: 900, endMs: 100 },
		{ startMs: 0, endMs: 11000 },
		{ startMs: 0, endMs: 100, depth: 1.5 },
		{ startMs: 0, endMs: 100, cx: 2 },
	])
		assert.throws(() => applyCommand(state(), "add_zoom_region", args));
});
test("project loading validates versions, preserves edits and resets playhead", () => {
	const s = state();
	s.playhead = 5;
	assert.equal(applyCommand(s, "open_project", { project: s.project }).playhead, 0);
	assert.throws(() =>
		applyCommand(s, "open_project", { project: { ...s.project, version: 99 } }),
	);
});
test("caption HTML is escaped and output deterministic", () => {
	const s = applyCommand(state(), "set_caption", { text: '<script>alert("x")</script>' });
	const html = toHyperFrames(s.project);
	assert.ok(html.includes("&lt;script&gt;"));
	assert.equal(html, toHyperFrames(s.project));
	assert.ok(html.includes('data-composition-id="subtake"'));
});
test("p95 uses nearest rank, not a rounded average", () => {
	assert.equal(percentile(Array.from({ length: 100 }, (_, i) => i + 1)), 95);
	assert.throws(() => percentile([]));
});
