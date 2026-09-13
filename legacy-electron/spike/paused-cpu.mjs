import { readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
const folder = resolve(process.argv[2]);
const config = JSON.parse(await readFile(resolve(folder, "connection.json"), "utf8"));
const env = JSON.parse(await readFile(resolve(folder, "environment.json"), "utf8"));
function snapshot() {
	const rows = execFileSync("ps", ["-axo", "pid=,ppid=,time=,comm="], { encoding: "utf8" })
		.trim()
		.split("\n")
		.map((line) => {
			const [pid, ppid, time, ...name] = line.trim().split(/\s+/);
			const parts = time.split(":").map(Number);
			return {
				pid: +pid,
				ppid: +ppid,
				seconds: parts.reduce((a, b) => a * 60 + b, 0),
				name: name.join(" "),
			};
		});
	const ids = new Set([env.servicePid, env.shellPid]);
	let n = 0;
	while (n !== ids.size) {
		n = ids.size;
		for (const r of rows) if (ids.has(r.ppid)) ids.add(r.pid);
	}
	return rows.filter((r) => ids.has(r.pid) && !/(^|\/)ps$/.test(r.name));
}
const reports = [];
for (const suspended of [false, true]) {
	const response = await fetch(`${config.origin}/api/command`, {
		method: "POST",
		headers: { "Content-Type": "application/json", Authorization: `Bearer ${config.token}` },
		body: JSON.stringify({ name: "set_preview_suspended", args: { suspended } }),
	});
	if (!response.ok) throw Error(await response.text());
	await new Promise((r) => setTimeout(r, 3000));
	const start = Date.now();
	const before = snapshot();
	await new Promise((r) => setTimeout(r, 30000));
	const after = snapshot();
	const elapsed = (Date.now() - start) / 1000;
	const processes = after.map((r) => ({
		...r,
		deltaCpuSeconds: r.seconds - (before.find((p) => p.pid === r.pid)?.seconds ?? r.seconds),
	}));
	reports.push({
		suspended,
		elapsedSeconds: elapsed,
		percentOfOneCore: (100 * processes.reduce((s, p) => s + p.deltaCpuSeconds, 0)) / elapsed,
		processes,
	});
}
await fetch(`${config.origin}/api/command`, {
	method: "POST",
	headers: { "Content-Type": "application/json", Authorization: `Bearer ${config.token}` },
	body: JSON.stringify({ name: "set_preview_suspended", args: { suspended: false } }),
});
await writeFile(resolve(folder, "paused-cpu.json"), JSON.stringify(reports, null, 2));
console.log(JSON.stringify(reports));
