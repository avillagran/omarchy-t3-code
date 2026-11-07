// Fixture-only evidence. Nothing is exposed to the page's main world.
const {ipcRenderer}=require('electron');
ipcRenderer.send('owned-sandbox-proof',{sandboxed:process.sandboxed,isolated:process.contextIsolated,electron:process.versions.electron});
