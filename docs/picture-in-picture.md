# Picture-in-Picture in Blue Web

Watch a video in a small floating frame that stays above every other window,
like Picture-in-Picture on Android.

## Using it

* Open a page with a video and press the **PiP button** in the status strip at
  the bottom of Blue Web, **Ctrl+Shift+P**, or the PiP button in the window's
  title bar (when the page has a video, that button floats just the video
  instead of shrinking the whole browser window).
* **Automatic:** if a video is playing and you switch to another tab or
  minimise Blue Web / change workspace, it carries on in Picture-in-Picture.
  Turn this off in *Blue Web → Settings → Automatic Picture-in-Picture*.
* In the frame: play/pause, back/forward 10 s, mute. Drag the title bar to
  move (it snaps to the nearest screen corner), drag the small grip in the
  corner facing the screen centre to resize. Double-click the title bar or
  press ⤢ to go back to the tab (the browser window is reopened if it was
  minimised or closed); × closes the video.
* One video at a time. Closing the Blue Web window ends its Picture-in-Picture.

## How it works

The page is **not** copied. The same embedded webview that shows the tab is
moved into the frame, and a script injected into the page stretches the chosen
`<video>` over the whole view and hides everything else (reversibly — leaving
Picture-in-Picture removes every attribute and style it added). The video keeps
its position, sound and subtitles, and nothing reloads.

* `src-tauri/src/BlueWebApp/pip.rs` — the injected script, the allow-list of
  player commands (`play`, `pause`, `toggle`, `mute`, `unmute`, `toggle_mute`,
  `seek`) and unit tests.
* `src-tauri/src/BlueWebApp/mod.rs` — `web_pip_report`, `web_pip_enter`,
  `web_pip_exit`, `web_pip_control`.
* `src/lib/components/apps/Blue-Web/pip.ts` — state, geometry (unit-tested),
  hand-over between window instances.
* `src/lib/components/apps/Blue-Web/WebPipOverlay.svelte` — the frame, mounted
  once in `App.svelte`.

## Security

The controls live in the shell, not in the page. The only command a web page
may call is `web_pip_report` (permission `allow-web-pip-report`), which says
"I have a video / it plays / it is muted / its aspect ratio". The backend checks
that the caller is the webview of the tab named in the report and clamps the
numbers. A page cannot start Picture-in-Picture, move the frame or call
anything else.

## Limits

* Videos inside a cross-origin `<iframe>` (for example an embedded player on a
  blog) are out of reach of the page script, so those pages report no video.
* The native page cannot be partially covered by shell DOM: while a menu or
  dialog is open the video is hidden, and other Blue Web pages are clipped
  around the frame.
* Moving the page between a tab and the frame needs the Linux embedding in
  `linux_embed.rs`; on other platforms bounds are set with the normal webview
  API, but this has only been exercised in tests, not on a running desktop.
