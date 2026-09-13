import { invoke } from "@tauri-apps/api/core";
import type { EditorProjectData } from "../src/components/video-editor/projectPersistence";
export interface Platform {
	openVideo(): Promise<{ path: string; url: string }>;
	getLocalMediaUrl(path: string): string;
	saveProject(project: EditorProjectData): Promise<unknown>;
	loadProject(): Promise<EditorProjectData>;
	showHud(): Promise<unknown>;
	startRecording(): Promise<never>;
	exportVideo(
		frames: string[],
	): Promise<{
		path: string;
		bytes: number;
		frames: number;
		fps: number;
		audio: boolean;
		backend: string;
	}>;
}
declare global {
	interface Window {
		spikeHost?: { showHud(): Promise<unknown> };
	}
}
export const params = new URLSearchParams(location.search);
export const token = params.get("token") ?? "";
export async function request(operation: string, payload: unknown = {}) {
	const response = await fetch(`/api/${operation}`, {
		method: "POST",
		headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
		body: JSON.stringify(payload),
	});
	const result = await response.json();
	if (!response.ok) throw Error(result.error);
	return result;
}
const common = {
	openVideo: () => request("open"),
	getLocalMediaUrl: (_path: string) => `/media?token=${encodeURIComponent(token)}`,
	saveProject: (project: EditorProjectData) => request("save", project),
	loadProject: () => request("load"),
	startRecording: async (): Promise<never> => {
		throw Error("Recording helper migration is outside this reference slice");
	},
	exportVideo: (frames: string[]) => request("export", { frames }),
};
export const electronAdapter: Platform = { ...common, showHud: () => window.spikeHost!.showHud() };
export const tauriAdapter: Platform = { ...common, showHud: () => invoke("show_hud") };
export const platform = params.get("shell") === "tauri" ? tauriAdapter : electronAdapter;
