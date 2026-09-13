import { spawn, execFileSync } from "node:child_process";
import { mkdir, writeFile, stat, readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { startService, execute } from "./server.mjs";
const root = fileURLToPath(new URL(".", import.meta.url));
const shell = process.argv[2];
if (!["electron", "tauri"].includes(shell))
	throw Error(
		"Usage: node run.mjs electron|tauri [--auto] [--exit] [--idle=600] [--media=/absolute/file.mp4]",
	);
const flags = process.argv.slice(3);
const auto = flags.includes("--auto");
const idle = Number(flags.find((s) => s.startsWith("--idle="))?.split("=")[1] ?? 0);
if (!Number.isFinite(idle) || idle < 0 || idle > 3600) throw Error("Idle must be 0..3600 seconds");
const id = `${shell}-${new Date().toISOString().replaceAll(":", "-")}`;
const work = resolve(root, "results", id);
await mkdir(work, { recursive: true });
let media =
	flags.find((s) => s.startsWith("--media="))?.slice(8) ?? resolve(root, "work/fixture.mp4");
if (!flags.some((s) => s.startsWith("--media="))) {
	await mkdir(resolve(root, "work"), { recursive: true });
	try {
		await stat(media);
	} catch {
		await execute("ffmpeg", [
			"-hide_banner",
			"-loglevel",
			"error",
			"-f",
			"lavfi",
			"-i",
			"testsrc2=size=1920x1080:rate=30",
			"-f",
			"lavfi",
			"-i",
			"sine=frequency=440:sample_rate=48000",
			"-t",
			"10",
			"-c:v",
			"libx264",
			"-preset",
			"fast",
			"-crf",
			"18",
			"-pix_fmt",
			"yuv420p",
			"-g",
			"60",
			"-c:a",
			"aac",
			"-movflags",
			"+faststart",
			media,
		]);
	}
}
const source = execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim();
const service = await startService({ media, work });
const config = { origin: service.origin, token: service.token };
await writeFile(resolve(work, "connection.json"), JSON.stringify(config), { mode: 0o600 });
const launchedAt = Date.now();
const url = new URL(service.origin);
for (const [k, v] of Object.entries({
	shell,
	token: service.token,
	launchedAt: String(launchedAt),
	auto: auto ? "1" : "0",
}))
	url.searchParams.set(k, v);
const executable =
	shell === "electron"
		? createRequire(import.meta.url)("electron")
		: resolve(root, "src-tauri/target/release/subtake-shell-spike");
const child = spawn(executable, shell === "electron" ? [resolve(root, "electron.cjs")] : [], {
	env: { ...process.env, SUBTAKE_SPIKE_URL: url.href },
	stdio: ["ignore", "inherit", "inherit"],
});
let exitCode;
child.on("exit", (code) => {
	exitCode = code ?? 1;
	service.close();
});
child.on("error", (e) => {
	console.error(e);
	exitCode = 1;
});
console.log(`Running ${shell}; results: ${work}`);
const assets = await readdir(resolve(root, "dist/assets"));
const frontendSha256 = createHash("sha256");
for (const name of assets.filter((name) => name.endsWith(".js")).sort())
	frontendSha256.update(await readFile(resolve(root, "dist/assets", name)));
const metadata = {
	frontendSha256: frontendSha256.digest("hex"),
	id,
	shell,
	source,
	platform: process.platform,
	arch: process.arch,
	os: execFileSync("sw_vers", ["-productVersion"], { encoding: "utf8" }).trim(),
	mediaSha256: createHash("sha256")
		.update(await readFile(media))
		.digest("hex"),
	launchedAt,
	servicePid: process.pid,
	shellPid: child.pid,
	build: "release frontend; release Tauri binary; stock Electron binary",
	measurement:
		"warm filesystem launch, not cold boot; synthetic sequential seek + 2 RAF + render submit, not physical input-to-photon",
};
await writeFile(resolve(work, "environment.json"), JSON.stringify(metadata, null, 2));
const finish = () => {
	child.kill("SIGTERM");
	service.close();
};
process.on("SIGINT", () => {
	finish();
	process.exit(130);
});
process.on("SIGTERM", () => {
	finish();
	process.exit(143);
});
if (auto) {
	const deadline = Date.now() + 180000;
	while (
		!service.reports.some((r) => ["complete", "error"].includes(r.phase)) &&
		exitCode === undefined &&
		Date.now() < deadline
	)
		await new Promise((r) => setTimeout(r, 250));
	const final = service.reports.find((r) => ["complete", "error"].includes(r.phase));
	if (!final || final.phase === "error" || !final.roundTrip || final.hud?.error) {
		finish();
		throw Error(final?.error ?? `Shell exited or timed out (${exitCode})`);
	}
	console.log(JSON.stringify(final));
	if (idle) {
		const samples = [];
		const start = Date.now();
		while (Date.now() - start < idle * 1000 && exitCode === undefined) {
			// Process-tree RSS is explicitly a lower bound on macOS: WKWebView XPC processes may be reparented.
			const rows = execFileSync("ps", ["-axo", "pid=,ppid=,rss=,pcpu=,time=,comm="], {
				encoding: "utf8",
			})
				.trim()
				.split("\n")
				.map((line) => {
					const [pid, ppid, rss, cpu, time, ...name] = line.trim().split(/\s+/);
					return {
						pid: +pid,
						ppid: +ppid,
						rssKiB: +rss,
						cpuPercent: +cpu,
						cpuTime: time,
						name: name.join(" "),
					};
				});
			const ids = new Set([process.pid, child.pid]);
			let count = 0;
			while (count !== ids.size) {
				count = ids.size;
				for (const row of rows) if (ids.has(row.ppid)) ids.add(row.pid);
			}
			const tree = rows.filter((r) => ids.has(r.pid) && !r.name.endsWith("/ps"));
			samples.push({
				elapsedMs: Date.now() - start,
				rssKiB: tree.reduce((sum, r) => sum + r.rssKiB, 0),
				processes: tree,
				unattributedWebKit: rows.filter((r) => /WebKit/.test(r.name) && !ids.has(r.pid)),
			});
			await new Promise((r) => setTimeout(r, 5000));
		}
		await writeFile(
			resolve(work, "idle.json"),
			JSON.stringify(
				{
					durationSeconds: (Date.now() - start) / 1000,
					requestedDurationSeconds: idle,
					interrupted: exitCode !== undefined,
					scope: "service and descendant RSS; does not attribute WebKit XPC or shared memory, not a total physical-memory comparison",
					samples,
				},
				null,
				2,
			),
		);
	}
	if (flags.includes("--exit")) finish();
} else child.on("exit", () => service.close());
