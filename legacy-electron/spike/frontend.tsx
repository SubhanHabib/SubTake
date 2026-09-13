import React, { useRef, useState, useEffect, useCallback } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import VideoPlayback, { type VideoPlaybackRef } from "../src/components/video-editor/VideoPlayback";
import {
	normalizeProjectEditor,
	type EditorProjectData,
} from "../src/components/video-editor/projectPersistence";
import { applyCommand, initialProject, percentile, toHyperFrames } from "./core.mjs";
import { platform, params, request, token } from "./platform";
import "../src/index.css";
import "./style.css";
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
const paint = () =>
	new Promise<void>((resolve) =>
		requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
	);
async function until(check: () => boolean, timeout = 20000) {
	const start = performance.now();
	while (!check()) {
		if (performance.now() - start > timeout) throw Error("Preview readiness timed out");
		await sleep(20);
	}
}
function App() {
	const preview = useRef<VideoPlaybackRef>(null);
	const [state, setState] = useState({
		project: initialProject("fixture.mp4") as EditorProjectData,
		playhead: 0,
		duration: 0,
		revision: 0,
	});
	const current = useRef(state);
	const ready = useRef(false);
	const [url, setUrl] = useState("");
	const [message, setMessage] = useState("Loading fixture…");
	const [playing, setPlaying] = useState(false);
	const [suspended, setSuspended] = useState(false);
	const [thumbs, setThumbs] = useState<string[]>([]);
	const queue = useRef(Promise.resolve<unknown>(null));
	const editor = normalizeProjectEditor(state.project.editor);
	const update = (next: typeof state) => {
		current.current = next;
		flushSync(() => setState(next));
	};
	const seek = async (seconds: number) => {
		const video = preview.current?.video;
		if (!video) throw Error("No video");
		const seekStart = performance.now();
		const next = applyCommand(current.current, "set_playhead", { seconds });
		const pending =
			Math.abs(video.currentTime - seconds) > 0.001
				? new Promise<void>((resolve, reject) => {
						const done = () => {
							clearTimeout(timer);
							video.removeEventListener("seeked", done);
							resolve();
						};
						const timer = setTimeout(() => {
							video.removeEventListener("seeked", done);
							reject(Error("Seek timed out"));
						}, 10000);
						video.addEventListener("seeked", done);
						video.currentTime = seconds;
					})
				: Promise.resolve();
		update(next);
		await pending;
		const mediaSeekMs = performance.now() - seekStart;
		await paint();
		const app = preview.current?.app;
		if (!app) throw Error("No Pixi renderer");
		app.renderer.render(app.stage);
		return { mediaSeekMs, seekAndPaintMs: performance.now() - seekStart };
	};
	const capture = useCallback(async () => {
		const app = preview.current?.app;
		if (!app) throw Error("No Pixi renderer");
		const source = await app.renderer.extract.canvas({ target: app.stage });
		const canvas = document.createElement("canvas");
		canvas.width = 960;
		canvas.height = 540;
		canvas.getContext("2d")!.drawImage(source as HTMLCanvasElement, 0, 0, 960, 540);
		return canvas.toDataURL("image/png");
	}, []);
	const run = async (name: string, args: Record<string, unknown> = {}) => {
		if (name === "get_state") return current.current;
		if (name === "set_preview_suspended") {
			if (typeof args.suspended !== "boolean") throw Error("Expected boolean");
			flushSync(() => setSuspended(args.suspended as boolean));
			await paint();
			return { suspended: args.suspended };
		}
		if (name === "set_playhead") {
			const timing = await seek(args.seconds as number);
			return { seconds: current.current.playhead, ...timing };
		}
		if (name === "render_preview") {
			await seek((args.seconds ?? current.current.playhead) as number);
			return { image: await capture() };
		}
		if (name === "save_project") return platform.saveProject(current.current.project);
		if (name === "show_hud") return platform.showHud();
		if (name === "hyperframes")
			return request("hyperframes", { html: toHyperFrames(current.current.project) });
		if (name === "export_video") {
			// Diagnostic frame export. Explicitly refuse layers this Pixi-only capture does not include.
			const e = current.current.project.editor;
			if (
				e.autoCaptions?.length ||
				e.annotationRegions?.length ||
				e.audioRegions?.length ||
				e.webcam?.enabled ||
				e.trimRegions?.length ||
				e.speedRegions?.length ||
				e.clipRegions?.length
			)
				throw Error(
					"Diagnostic export supports only the single video and Pixi zoom; remove captions/annotations/audio/webcam/edits first",
				);
			const frames: string[] = [];
			const before = current.current.playhead;
			const start = performance.now();
			try {
				for (let frame = 0; frame < 30; frame++) {
					await seek(frame / 15);
					frames.push(await capture());
				}
				const output = await platform.exportVideo(frames);
				return { ...output, elapsedMs: performance.now() - start };
			} finally {
				await seek(before);
			}
		}
		if (name === "open_project") {
			const project = (args.project ?? (await platform.loadProject())) as EditorProjectData;
			if (project.videoPath !== current.current.project.videoPath)
				throw Error(
					"This fixture-scoped spike cannot relink another media file; restart with --media",
				);
			update(applyCommand(current.current, name, { project }));
			await seek(0);
			return { revision: current.current.revision };
		}
		update(applyCommand(current.current, name, args));
		await paint();
		return { revision: current.current.revision };
	};
	const dispatch = (name: string, args: Record<string, unknown> = {}) => {
		const job = queue.current.then(() => run(name, args));
		queue.current = job.catch(() => {
			/* Keep the command queue usable; the caller receives the error. */
		});
		return job;
	};
	const dispatchRef = useRef(dispatch);
	dispatchRef.current = dispatch;
	const action = (name: string, args?: Record<string, unknown>) =>
		void dispatchRef
			.current(name, args)
			.then((r) => setMessage(JSON.stringify(r).slice(0, 240)))
			.catch((e) => setMessage(String(e)));
	useEffect(() => {
		let ws: WebSocket | undefined;
		let disposed = false;
		(async () => {
			const media = await platform.openVideo();
			if (disposed) return;
			current.current.project = initialProject(media.path);
			setState({ ...current.current });
			setUrl(media.url);
			await until(
				() =>
					!!(
						ready.current &&
						preview.current?.app &&
						(preview.current?.video?.readyState ?? 0) >= 2
					),
			);
			await paint();
			preview.current!.app!.renderer.render(preview.current!.app!.stage);
			const firstPreviewMs = Date.now() - Number(params.get("launchedAt"));
			const video = preview.current!.video!;
			const webgpu = !!navigator.gpu;
			let adapter = false;
			try {
				adapter = !!(await navigator.gpu?.requestAdapter());
			} catch {
				/* Unsupported capability is reported as false. */
			}
			let h264Encode = false;
			try {
				h264Encode = !!(
					await window.VideoEncoder?.isConfigSupported({
						codec: "avc1.42001f",
						width: 960,
						height: 540,
						bitrate: 2000000,
						framerate: 30,
					})
				)?.supported;
			} catch {
				/* Unsupported capability is reported as false. */
			}
			const capabilities = {
				userAgent: navigator.userAgent,
				webgpu,
				gpuAdapter: adapter,
				videoEncoder: typeof window.VideoEncoder,
				videoDecoder: typeof window.VideoDecoder,
				h264Encode,
				requestVideoFrameCallback: typeof video.requestVideoFrameCallback,
				pixiRendererType: preview.current!.app!.renderer.type,
			};
			await request("report", { phase: "ready", firstPreviewMs, capabilities });
			const snapshots = [];
			for (let i = 0; i < 8; i++) {
				await dispatchRef.current("set_playhead", {
					seconds: ((video.duration - 0.1) * i) / 8,
				});
				snapshots.push(await capture());
			}
			setThumbs(snapshots);
			await dispatchRef.current("set_playhead", { seconds: 0 });
			await request("report", {
				phase: "usable",
				usableMs: Date.now() - Number(params.get("launchedAt")),
			});
			ws = new WebSocket(`ws://${location.host}/ws?token=${encodeURIComponent(token)}`);
			ws.onmessage = async (event) => {
				const command = JSON.parse(event.data);
				try {
					const result = await dispatchRef.current(command.name, command.args);
					ws?.send(JSON.stringify({ id: command.id, result }));
				} catch (e) {
					ws?.send(JSON.stringify({ id: command.id, error: String(e) }));
				}
			};
			setMessage("Ready. Uses the production VideoPlayback and project normalizer.");
			if (params.get("auto") === "1") {
				await dispatchRef.current("add_zoom_region", {
					startMs: 1000,
					endMs: 7000,
					depth: 2,
				});
				const samples = [];
				const mediaSeekSamplesMs = [];
				for (let i = 0; i < 40; i++) {
					const start = performance.now();
					const timing = (await dispatchRef.current("set_playhead", {
						seconds: (((i * 37) % 97) / 100) * (video.duration - 0.1),
					})) as { mediaSeekMs: number };
					samples.push(performance.now() - start);
					mediaSeekSamplesMs.push(timing.mediaSeekMs);
				}
				await dispatchRef.current("save_project");
				const saved = JSON.stringify(current.current.project);
				await dispatchRef.current("open_project");
				const roundTrip = saved === JSON.stringify(current.current.project);
				let hud: unknown;
				try {
					hud = await dispatchRef.current("show_hud");
					await sleep(500);
					await dispatchRef.current("show_hud");
				} catch (e) {
					hud = { error: String(e) };
				}
				const output = await dispatchRef.current("export_video");
				await dispatchRef.current("set_caption", {
					text: "SubTake shell spike",
					startMs: 0,
					endMs: 2000,
				});
				const hyperframes = await dispatchRef.current("hyperframes");
				await dispatchRef.current("set_playhead", { seconds: 0 });
				const quality = video.getVideoPlaybackQuality?.();
				const result = {
					phase: "complete",
					mediaSeekSamplesMs,
					mediaSeekP95Ms: percentile(mediaSeekSamplesMs),
					scrubSamplesMs: samples,
					scrubP95Ms: percentile(samples),
					scrubMedianMs: percentile(samples, 0.5),
					roundTrip,
					hud,
					export: output,
					hyperframes,
					quality: quality
						? { dropped: quality.droppedVideoFrames, total: quality.totalVideoFrames }
						: null,
				};
				await request("report", result);
				setMessage(
					`Complete: p95 seek + paint ${result.scrubP95Ms.toFixed(1)}ms. Raw results saved.`,
				);
			}
		})().catch(async (e) => {
			setMessage(String(e));
			await request("report", { phase: "error", error: String(e) }).catch(() => {
				/* Keep the command queue usable; the caller receives the error. */
			});
		});
		return () => {
			disposed = true;
			ws?.close();
		};
	}, [capture]);
	return (
		<main>
			<h1>SubTake · {params.get("shell")} reference slice</h1>
			<nav>
				<button
					onClick={() => {
						if (playing) preview.current?.pause();
						else void preview.current?.play();
					}}
				>
					Play / pause
				</button>
				<button
					onClick={() =>
						action("add_zoom_region", { startMs: 1000, endMs: 7000, depth: 2 })
					}
				>
					Add zoom
				</button>
				<button onClick={() => action("save_project")}>Save project</button>
				<button onClick={() => action("open_project")}>Load project</button>
				<button onClick={() => action("show_hud")}>Toggle HUD</button>
				<button onClick={() => action("export_video")}>Export 2s diagnostic</button>
				<button onClick={() => action("hyperframes")}>HyperFrames title card</button>
			</nav>
			<div className="preview">
				{url && (
					<VideoPlayback
						ref={preview}
						suspendRendering={suspended}
						videoPath={url}
						currentTime={state.playhead}
						isPlaying={playing}
						onDurationChange={(duration) => {
							current.current = { ...current.current, duration };
							setState(current.current);
						}}
						onPreviewReadyChange={(value) => {
							ready.current = value;
						}}
						onTimeUpdate={(time) => {
							current.current = { ...current.current, playhead: time };
							setState(current.current);
						}}
						onPlayStateChange={setPlaying}
						onError={setMessage}
						zoomRegions={editor.zoomRegions}
						selectedZoomId={null}
						onSelectZoom={() => {
							/* Selection tools are outside this reference slice. */
						}}
						onZoomFocusChange={() => {
							/* Commands own the test zoom focus. */
						}}
						wallpaper="#111827"
						padding={0}
						borderRadius={0}
						aspectRatio="16:9"
						showCursor={false}
						zoomMotionBlur={0}
					/>
				)}
			</div>
			<input
				aria-label="Timeline"
				type="range"
				min="0"
				max={Math.max(0, state.duration - 0.05)}
				step="0.01"
				value={state.playhead}
				onChange={(e) => action("set_playhead", { seconds: Number(e.target.value) })}
			/>
			<div className="strip">
				{thumbs.map((src, i) => (
					<img key={i} src={src} alt={`Cached frame ${i + 1}`} />
				))}
			</div>
			<p role="status">{message}</p>
		</main>
	);
}
createRoot(document.getElementById("root")!).render(
	params.has("hud") ? (
		<main>
			<strong>SubTake HUD boundary</strong>
			<p>Always-on-top test window</p>
		</main>
	) : (
		<App />
	),
);
