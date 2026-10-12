# Design system — "Pro Studio Dark" (UI Option 1)

PulseCut's own visual language for a conventional professional editing layout. It follows layout *principles* common to pro editors (media left, viewer centre, inspector right, full-width timeline below) without copying any product's branding, iconography or pixel layout.

Tokens live in `apps/desktop/src/index.css` (`@theme`, Tailwind v4) and are used as `bg-pc-*`, `text-pc-*`, `border-pc-*`.

## Colour tokens

| Token | Value | Use |
|---|---|---|
| `pc-bg` | `#0e0f11` | App background, timeline lanes, input fields |
| `pc-panel` | `#15171a` | Panel bodies |
| `pc-panel-2` | `#1b1e22` | Panel headers, track headers, menus |
| `pc-raised` | `#23272c` | Hover, secondary buttons |
| `pc-border` / `pc-border-strong` | `#2a2e34` / `#3a3f47` | 1px dividers / emphasised dividers |
| `pc-text` / `pc-muted` / `pc-faint` | `#e6e8eb` / `#9aa3ad` / `#6b737d` | Text hierarchy |
| `pc-accent` / `pc-accent-hi` / `pc-accent-lo` | `#2f8cff` / `#5aa8ff` / `#1d4f8f` | Electric-blue focus, playhead, primary action, active toggles |
| `pc-danger` / `pc-warn` / `pc-ok` | `#ff5c5c` / `#f2b84b` / `#3ecf8e` | Errors and offline media / snapping indicator and warnings / verified |
| `pc-clip-video(-hi)` | `#2b4a72` / `#3a6aa6` | Video clip body / border |
| `pc-clip-audio(-hi)` | `#1e5a4d` / `#2b8c75` | Audio clip body / border |

Contrast: body text `#e6e8eb` on `#15171a` ≈ 14:1; muted `#9aa3ad` on `#15171a` ≈ 7:1; faint text (`#6b737d`, ≈ 3.9:1) is reserved for secondary hints, never for required information.

## Typography

- UI: `Inter`, `Segoe UI Variable`, `Segoe UI`, `system-ui` (no bundled fonts). Base 13px; panel titles 11px semibold uppercase with letter-spacing; menu items 12.5px.
- Numbers and timecode: `Cascadia Mono`, `JetBrains Mono`, `Consolas`, monospace with tabular figures (`.tabular`).

## Shape, spacing, motion

- Radius `--radius-pc: 3px` (controls, clips); dialogs 5px.
- Control heights: 28px (`h-7`) buttons and icon buttons, 24px numeric fields, 36px top bar, 32px panel headers, 36px timeline toolbar.
- Panel dividers are 1px with a 7px invisible hit area (`Splitter`).
- Motion: colour transitions only (~150ms). `prefers-reduced-motion` and **View › Toggle Reduced Motion** disable transitions.

## Layout

```
┌ Top command strip: logo · menus · project title · undo/redo · Import · Export ┐
├ Project Media ┃          Program viewer           ┃      Inspector          ┤
│ (280px, 200–560) ┃  canvas · safe areas · transport  ┃ (300px, 200–560)     │
├━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ splitter ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┤
│ Timeline (320px, ≥170): toolbar · ruler · track headers (172px) · lanes      │
└ Status bar: engine · project path · saved state ┘
```

- Panel sizes persist in `localStorage` (`pulsecut.layout.v1`); **Window › Reset Panel Layout** restores defaults. Track heights persist per track.
- Minimum window 1280×720 (configured in `tauri.conf.json`); defaults are chosen so 1366×768 leaves ≥ 450px for the viewer.
- High-DPI: canvases (viewer, ruler, waveforms) are sized by `devicePixelRatio`.

## Components

| Component | File | Notes |
|---|---|---|
| `IconButton`, `Button` | `components/ui.tsx` | Every icon button has an `aria-label` and tooltip with its shortcut |
| `NumberField` | `components/ui.tsx` | Slider + numeric input; commits on Enter/blur/slider release; Escape reverts; reset button |
| `Modal` | `components/ui.tsx` | Focus moves into the dialog and returns on close; Escape closes |
| `PanelHeader` | `components/ui.tsx` | |
| `Splitter` | `components/Splitter.tsx` | Pointer drag + arrow keys (Shift = larger step) |
| Menu bar | `components/TopBar.tsx` | `role=menubar/menu/menuitem`; arrow-key navigation; items generated from `lib/actions.ts` |
| Icons | `components/Icons.tsx` | Project-owned 24px stroke icons |

## Rules

- **No dead controls.** A visible enabled control must do what it says. Unimplemented features appear only as clearly labelled *planned* menu text (e.g. Effects menu) or not at all.
- Every action available from a menu has the same implementation as its shortcut and toolbar button (`ACTIONS` registry).
- Errors are shown in place (viewer banner, export dialog) or as toasts with the underlying message; nothing fails silently.
- Focus is always visible (`:focus-visible` 2px accent outline).

## Planned

Workspace presets (Editing / Color / Audio / Effects / Graphics) once those workspaces have real functionality; light theme is not planned.
