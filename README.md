# SolarOffice

A modern office suite in Rust — SolarWord, SolarGrid, SolarDeck, SolarDraw, SolarPDF,
SolarMarkdown and SolarHTML — with AI built in.

**Stack:** Dioxus 0.7 (desktop now, WASM later) + TailwindCSS v4 · Yrs (CRDT) for collaboration ·
Axum/Tokio sync server with PostgreSQL. No JavaScript/TypeScript in the app itself.

**Status — v0.1.0: the startup (Home) screen.** The module editors and the sync server follow.

## The Home screen

- **Tab strip = title bar**: Home tab, one tab per open file, **+** (new-file menu), theme switch.
  macOS traffic lights inset; drag the strip to move the window, double-click to zoom.
- **Sidebar**: Recent and Starred (with counts), Ask AI, the folder tree (save folder first, then
  folders you add; ⋯ → Open in Finder, New folder, Remove from sidebar), Settings.
- **Greeting** with your name, and **quick-start cards** for the seven modules plus *Open local
  file*. A new file opens in a tab and is written on first save (as in GenOffice).
- **File list**: type filter pills, search, refresh, item count; sortable Name / Opened / Modified /
  Size; star on hover; ⋯ or right-click menu (Open, Reveal in Finder, Copy path, Rename — keeps the
  extension, Move to folder…, Duplicate, Star, Remove from list, Move to Trash with confirm);
  checkboxes and ⌘-click for multi-select with a selection bar; missing files are marked.
- **Folder view**: click a sidebar folder to list its subfolders and documents.
- **Settings**: theme (System / Light / Dark), name for the greeting, save folder; About.
- First launch copies the *Project Sunrise* examples into `~/Documents/SolarOffice/Examples`.
- State is saved in `~/Library/Application Support/SolarOffice/home.json`
  (`SOLAROFFICE_CONFIG_DIR` overrides).

Until the editors arrive, a file tab shows the file and offers *Open in default app*.

### Shortcuts

| | |
|---|---|
| ⌘T / ⌘⇧T | new-file menu / reopen closed tab |
| ⌘W | close tab |
| ⌘1 … ⌘8, ⌘9 | Home, tab n, last tab |
| ⌃Tab, ⌃⇧Tab, ⌘⌥→/← | next / previous tab |
| ⌘⇧1 … ⌘⇧7 | new SolarWord … SolarHTML file |
| ⌘N, ⌘O, ⌘F, ⌘, | new document, open, search, settings |

## Build and run

Needs Rust 1.88+ (`rustup update`).

```sh
cargo run -p solaroffice          # run (the compiled CSS is checked in)
./build.sh --run                  # same, release build
./build.sh --native               # SolarOffice.app + .dmg for this Mac, in dist/
./build.sh                        # universal (Apple silicon + Intel)
```

### Changing the styling

Classes are Tailwind utilities in the `rsx!` code; the theme tokens live in
`apps/solaroffice/tailwind.css`. Rebuild `apps/solaroffice/assets/tailwind.css` with either

```sh
cargo install dioxus-cli --version 0.7.10 --locked
cd apps/solaroffice && dx serve   # hot reload; dx runs Tailwind itself
```

or `cd apps/solaroffice && npm install && npm run css` (`npm run css:watch` while editing).

## Layout

```
apps/solaroffice/      the desktop app (Dioxus)
  src/main.rs          window setup
  src/state.rs         shared state (signals): tabs, page, menus, dialogs, toast
  src/files.rs         rename/duplicate/move/trash, Finder, clipboard, bundled examples
  src/icons.rs         line icons (Lucide) and module badges
  src/ui/              tabs, sidebar, home, menus & dialogs, Ask AI, module tab
  tailwind.css         Tailwind input + theme tokens; assets/tailwind.css is the output
crates/solar-core/     modules (AppKind), Recent/Starred library, settings, home.json
examples/              one example document per module
packaging/macos/       .app / .dmg packaging
```

## Next

1. Workspace crates for `solar-sync` (Yrs documents) and `solar-server` (Axum + PostgreSQL,
   Docker Compose).
2. The AI layer (providers, models, Ask AI answers).
3. The first editor module, with ONLYOFFICE-style ribbons and left/right toolbars.
