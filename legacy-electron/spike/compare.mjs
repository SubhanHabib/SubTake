import { spawn } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { percentile } from "./core.mjs";
const root = import.meta.dirname;
const selected = [];
for (let pair = 0; pair < 5; pair++) {
	for (const shell of pair % 2 ? ["tauri", "electron"] : ["electron", "tauri"]) {
		console.log(`Pair ${pair + 1}/5: ${shell}`);
		const stdout = await new Promise((resolve, reject) => {
			const child = spawn(process.execPath, [`${root}/run.mjs`, shell, "--auto", "--exit"], {
				stdio: ["ignore", "pipe", "pipe"],
			});
			let output = "";
			let errors = "";
			child.stdout.on("data", (b) => {
				output += b;
			});
			child.stderr.on("data", (b) => {
				errors += b;
			});
			child.on("error", reject);
			child.on("exit", (code) =>
				code === 0 ? resolve(output) : reject(Error(errors + output)),
			);
		});
		const folder = stdout.split("\n")[0].split("results: ")[1];
		const reports = JSON.parse(await readFile(resolve(folder, "renderer.json"), "utf8"));
		const environment = JSON.parse(await readFile(resolve(folder, "environment.json"), "utf8"));
		const ready = reports.find((r) => r.phase === "ready");
		const usable = reports.find((r) => r.phase === "usable");
		const complete = reports.find((r) => r.phase === "complete");
		selected.push({ shell, pair: pair + 1, folder, environment, ready, usable, complete });
	}
}
if (
	new Set(selected.map((r) => r.environment.mediaSha256)).size !== 1 ||
	new Set(selected.map((r) => r.environment.frontendSha256)).size !== 1
)
	throw Error("Mismatched input or frontend hashes");
const summary = {
	method: "Median of five per-shell run metrics; 40 sequential seeks each; paired alternating order; warm filesystem, not cold startup",
	selected,
	summary: {},
};
for (const shell of ["electron", "tauri"]) {
	const runs = selected.filter((r) => r.shell === shell);
	const metric = (fn) => percentile(runs.map(fn), 0.5);
	summary.summary[shell] = {
		runs: runs.length,
		firstPreviewMs: metric((r) => r.ready.firstPreviewMs),
		usableMs: metric((r) => r.usable.usableMs),
		mediaSeekP95Ms: metric((r) => r.complete.mediaSeekP95Ms),
		seekAndPaintP95Ms: metric((r) => r.complete.scrubP95Ms),
		exportMs: metric((r) => r.complete.export.elapsedMs),
		capabilities: runs[0].ready.capabilities,
		allRoundTrips: runs.every((r) => r.complete.roundTrip),
		allHuds: runs.every((r) => r.complete.hud && !r.complete.hud.error),
	};
}
await writeFile(resolve(root, "comparison.json"), JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary.summary, null, 2));
