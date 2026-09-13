# Recording on macOS

SubTake opens as a menu-bar recorder. Its icon has **Open** and **Quit** commands. Open restores the compact floating overlay; hiding the overlay leaves SubTake running. The editor is created as a separate window and appears when a video/project has loaded.

1. Click the **display/window button** on the overlay to choose a recording source. Refresh reloads the list and retains the selected source when it still exists.
2. The **microphone** button opens microphone selection and the system-audio switch. The **camera** button opens webcam/device settings. The **timer** button offers no delay, 3, 5 or 10 seconds.
3. Press the **red Record button**. The editor hides, the countdown runs, and recording starts. Cancel during the countdown returns to setup.
4. Use **Pause / Resume** and **Stop** on the same overlay. Stop finalizes the files and opens the editor with the recorded project loaded in Scene.

Recordings save automatically under **Movies/SubTake** on Mac (the system Videos directory on other platforms when available). The More menu lets you choose another recordings folder, open a video/project, browse Projects/recoveries, or return to an already loaded editor. There is no save dialog before each recording. Saving the edited project remains a separate editor action.

The global Record shortcut opens the recorder first. With the overlay already visible and a source selected, it starts recording; during capture it stops. The Pause shortcut pauses/resumes. Closing the editor retains its project in memory and leaves the menu-bar app running; Quit checks for unsaved changes. Recording and finalization must finish before quitting.

If no sources appear, allow SubTake in **System Settings → Privacy & Security → Screen & System Audio Recording**, follow any macOS relaunch prompt, then refresh. Camera and microphone permissions are separate. Discovery success, errors and cancellation all release the busy state.

## Acceptance checks

`scripts/launcher-smoke.py` exercises menu-bar startup, hiding/reopening without an editor, native source-control clicks, and each setup panel. Its optional `--capture-fixture` mode only accepts the generated **SubTake Capture Fixture** window. It checks countdown cancellation, screen-only recording, pause/resume/stop and the recorded-project handoff into the editor. Test preferences are isolated; microphone, camera and system audio are off in the capture test. The run record is `docs/launcher-validation.json`. The fixture includes a red window covering the blue recording target; sampled saved frames must contain the blue target and exclude the red occluder. Run GUI capture checks with the displays awake, for example `caffeinate -u -d python3 scripts/launcher-smoke.py --capture-fixture`.

These checks do not establish microphone/camera/system-audio synchronization, permission revocation, long-session behavior or Windows capture support.
