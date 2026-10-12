# PulseCut design system

Option 1, Pro Studio Dark, is an original PulseCut layout. It is not a copy of another editor's branding or screenshots.

## Color

| Token | Value | Use |
| --- | --- | --- |
| `--bg-0` | `#0c0d10` | Window background |
| `--bg-1` | `#14161b` | Panels |
| `--bg-2` | `#1b1e25` | Menus, raised controls |
| `--bg-3` | `#242833` | Hover |
| `--line` | `#2e3440` | 1px dividers |
| `--text` | `#e8eaef` | Primary text |
| `--muted` | `#a7b0bd` | Secondary text |
| `--faint` | `#7f8a99` | Metadata |
| `--accent` | `#3b82f6` | Primary action and focus |
| `--accent-dim` | `#1d4f91` | Pressed |
| `--danger` | `#ef5b5b` | Errors and the playhead |
| `--warn` | `#e2b340` | Stale preview |
| `--video` | `#24507f` | Video clips |
| `--audio` | `#1d6b4e` | Audio clips |

The same values are Tailwind colors `bg0`–`bg3`, `line`, `ink`, `muted`, `faint`, and `accent`.

## Type and shape

The UI uses the system stack: Segoe UI, Inter, Liberation Sans, system-ui. No font files are bundled. Controls use a 3px radius. Icons in the window are inline SVG drawn for PulseCut. The app icon is the same mark: a near-black square, a blue horizontal bar, and a light vertical bar.

## Layout

The window is a 40px command strip, a workspace, and a 26px status line.

The workspace is a vertical split. The upper row is the media browser, the program monitor, and the inspector, separated by 6px splitters. The lower row is the timeline. Default widths are 280px, 300px, and 280px. Limits are 200–460, 220–480, and 160–520. The sizes persist in `localStorage` under `pulsecut.layout.v1`. Window → Reset layout clears them.

The minimum window is 1100×680. The default is 1440×900. At 1366×768 the minimum still fits the three columns and the timeline. High-DPI scaling is left to the window system.

## Command strip

Menus are drawn in the window so shortcuts match across operating systems: File, Edit, View, Playback, Timeline, Effects, Window, Help. Effects opens the feature-status note. It does not present controls for filters that are not built.

Only commands that call the project model, the renderer, or a real dialog are enabled. Disabled items are undo, redo, split, delete, and unlink when they do not apply.

## Program monitor

The monitor shows the current proxy, a timecode, frame step, and play/pause. Safe areas are a toggle that draws guides. They do not change the export. A stale proxy stays on screen with a warning until Play renders the current timeline.

## Timeline

Video tracks are drawn with the higher track on top. Audio tracks sit below. Track headers mute and lock. There is no solo button. Clips show a thumbnail or a waveform when those jobs succeed. Trim handles and drag both commit through the same edit commands the menus use. Snap (N) pulls the ghost to clip edges and the playhead. Rust snaps the committed time to the sequence frame.

## Motion and focus

Hover and splitters are the only motion. `prefers-reduced-motion: reduce` disables transitions and animations. `:focus-visible` draws a 2px accent outline.

## Status language

Unfinished work is described as planned in Help → Feature status and in `docs/FEATURE_MATRIX.md`. The interface does not use a control that only pretends to edit.
