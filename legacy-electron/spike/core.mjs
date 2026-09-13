// This envelope is the existing v2 EditorProjectData format, not a second project format.
export function validateProject(project) {
	if (
		!project ||
		project.version !== 2 ||
		typeof project.videoPath !== "string" ||
		!project.videoPath ||
		!project.editor ||
		typeof project.editor !== "object" ||
		Array.isArray(project.editor)
	)
		throw Error("Expected a version 2 SubTake project");
	return structuredClone(project);
}
export const initialProject = (videoPath) => ({
	version: 2,
	videoPath,
	editor: { zoomRegions: [], autoCaptions: [] },
});
function number(value, min, max) {
	if (typeof value !== "number" || !Number.isFinite(value) || value < min || value > max)
		throw Error(`Number must be within ${min}..${max}`);
	return value;
}
export function applyCommand(state, name, args = {}) {
	const next = structuredClone(state);
	switch (name) {
		case "open_project":
			return {
				...next,
				project: validateProject(args.project),
				playhead: 0,
				revision: next.revision + 1,
			};
		case "set_playhead":
			next.playhead = number(args.seconds, 0, next.duration);
			break;
		case "add_zoom_region": {
			const startMs = number(args.startMs, 0, next.duration * 1000);
			const endMs = number(args.endMs, startMs + 1, next.duration * 1000);
			const depth = number(args.depth ?? 2, 1, 6);
			if (!Number.isInteger(depth)) throw Error("Depth must be an integer");
			next.project.editor.zoomRegions ??= [];
			next.project.editor.zoomRegions.push({
				id: `spike-zoom-${next.revision}`,
				startMs,
				endMs,
				depth,
				mode: "manual",
				focus: { cx: number(args.cx ?? 0.5, 0, 1), cy: number(args.cy ?? 0.5, 0, 1) },
			});
			break;
		}
		case "set_caption": {
			const startMs = number(args.startMs ?? 0, 0, next.duration * 1000);
			const endMs = number(args.endMs ?? 2000, startMs + 1, next.duration * 1000);
			if (typeof args.text !== "string" || args.text.length > 200)
				throw Error("Caption must be <=200 characters");
			next.project.editor.autoCaptions = [
				{ id: "spike-caption", startMs, endMs, text: args.text },
			];
			break;
		}
		default:
			throw Error(`Unknown state command: ${name}`);
	}
	next.revision++;
	return next;
}
export function percentile(values, p = 0.95) {
	if (!values.length || values.some((v) => !Number.isFinite(v)))
		throw Error("Expected finite samples");
	return [...values].sort((a, b) => a - b)[Math.max(0, Math.ceil(p * values.length) - 1)];
}
const escapeHtml = (value) =>
	value.replace(
		/[&<>"']/g,
		(c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c],
	);
// Deliberately a branded title-card projection, not a faithful full-timeline exporter.
export function toHyperFrames(project) {
	validateProject(project);
	const title = escapeHtml(project.editor.autoCaptions?.[0]?.text || "SubTake shell spike");
	return `<!doctype html><html lang="en"><head><meta charset="UTF-8"><title>SubTake</title><script src="gsap.min.js"></script><style>body{margin:0;font-family:Arial,sans-serif;color:white}#scene{position:relative;width:960px;height:540px;overflow:hidden;background:#111827}.clip{position:absolute;inset:60px;display:flex;flex-direction:column;justify-content:center}h1{font-size:48px;margin:20px 0;overflow-wrap:anywhere}p{font-size:24px;color:#93c5fd}</style></head><body><div id="scene" data-composition-id="subtake" data-width="960" data-height="540" data-duration="2"><section id="subtake-title-card" class="clip" data-start="0" data-duration="2"><p>SUBTAKE</p><h1>${title}</h1></section></div><script>window.__timelines = window.__timelines || {}; window.__timelines.subtake = gsap.timeline({paused:true});</script></body></html>`;
}
