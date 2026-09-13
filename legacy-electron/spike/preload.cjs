const { contextBridge, ipcRenderer } = require("electron");
contextBridge.exposeInMainWorld("spikeHost", {
	showHud: () => ipcRenderer.invoke("spike:show-hud"),
});
