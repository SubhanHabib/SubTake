const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("node:path");
let hud, editor;
const url = process.env.SUBTAKE_SPIKE_URL;
if (!url || !url.startsWith("http://127.0.0.1:")) throw Error("Launch with run.mjs");
app.whenReady().then(() => {
	editor = new BrowserWindow({
		width: 1100,
		height: 800,
		show: true,
		webPreferences: {
			preload: path.join(__dirname, "preload.cjs"),
			contextIsolation: true,
			nodeIntegration: false,
			sandbox: true,
		},
	});
	editor.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
	editor.webContents.on("will-navigate", (event, target) => {
		if (target !== url) event.preventDefault();
	});
	ipcMain.handle("spike:show-hud", (event) => {
		if (event.sender !== editor.webContents) throw Error("Editor only");
		if (hud && !hud.isDestroyed()) {
			hud.close();
			hud = null;
			return { visible: false };
		}
		hud = new BrowserWindow({
			width: 340,
			height: 110,
			alwaysOnTop: true,
			resizable: false,
			webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
		});
		const target = new URL(url);
		target.searchParams.set("hud", "1");
		hud.loadURL(target.href);
		return { visible: true };
	});
	editor.webContents.on("console-message", (event) => {
		if (event.level === "error" || event.level === "warning")
			console.error(event.message.slice(0, 500));
	});
	editor.loadURL(url);
});
app.on("window-all-closed", () => app.quit());
