# Thryve Topology

A cross-platform desktop viewer for [Thryve](https://github.com/thryveframework/thryve) task-topology graphs, built with [`egui`](https://github.com/emilk/egui)/[`eframe`](https://docs.rs/eframe), in the look of Thryve's website.

[![CI](https://github.com/thryveframework/thryve-topology/actions/workflows/ci.yml/badge.svg)](https://github.com/thryveframework/thryve-topology/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/thryveframework/thryve-topology)](https://github.com/thryveframework/thryve-topology/releases/latest)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](LICENSE)

## What this is

When a Thryve program runs with `is_debugging` and `visual_graph` both on in its config's `[debugging]` section, Thryve records which line of the program sent which task, and how many times. When the program shuts down, it writes that graph to `topology/topology_export.ron` in its root folder: the program's own folder, unless the config's `init.root_dir` names another.

**Thryve Topology is the viewer for that file.** It ships as a binary named `Topology`. At shutdown, Thryve looks for `Topology.exe` next to the program's own `.exe` (never in the root folder) and starts it with the file's full path, so the graph opens by itself. You can also open any export by hand.

The viewer:

- reads the file named on its command line; without one, it reads `topology/topology_export.ron` in its own folder and says in the window that it opened the default file;
- lays the graph out in columns, from the starting points on the left to the tasks they lead to on the right, with each separate part of the graph in its own band;
- shows each call site as a card (the task's name, its file and line, and how many tasks that line made), and each edge as an arrow with its count;
- explains a missing, empty or unreadable file in the window, instead of showing an empty canvas.

It's a companion to [Thryve](https://github.com/thryveframework/thryve): it doesn't run or instrument anything itself, it only shows what Thryve exported.

## Installing

### Download a prebuilt binary

Grab the archive for your platform from the [latest release](https://github.com/thryveframework/thryve-topology/releases/latest):

| Platform | Archive |
|---|---|
| Windows (x86_64) | `thryve-topology-<version>-windows-x86_64.zip` |
| Linux (x86_64) | `thryve-topology-<version>-linux-x86_64.tar.gz` |
| macOS (Intel) | `thryve-topology-<version>-macos-x86_64.tar.gz` |
| macOS (Apple Silicon) | `thryve-topology-<version>-macos-aarch64.tar.gz` |

Unpack it, and put `Topology` (`Topology.exe` on Windows) next to your Thryve program's executable. That's where Thryve looks for it: never in the root folder, since whoever can write to the root folder could put a program of their own there.

On Windows, the first start may show "Windows protected your PC", because `Topology.exe` isn't code-signed: choose **More info** → **Run anyway**.

Every release also gets an automatic **"Source code (zip)"** and **"(tar.gz)"** download from GitHub — no separate packaging needed for those.

### Build from source

Requires Rust 1.92 or later, the oldest that egui 0.34 builds with:

```bash
git clone https://github.com/thryveframework/thryve-topology.git
cd thryve-topology
cargo build --release --bin Topology
# binary at target/release/Topology (or Topology.exe on Windows)
```

## Usage

```text
Topology [file]
```

1. Run a Thryve program with graph mode on (see the [Thryve README](https://github.com/thryveframework/thryve#configuration) for the `[debugging]` and `[init]` settings), and let it shut down, so the export gets written.
2. Thryve starts `Topology` with the file's path. To open an export yourself, pass its path: `Topology D:\ThryveData\topology\topology_export.ron`.
3. In the viewer:
   - **Scroll** to zoom, **drag** the background to move around, **drag a card** to move it;
   - **click** a card or an arrow for its details in the panel on the right; **Esc** goes back to the overview;
   - **double-click** the background, press **F** or click **Fit** to show the whole graph; **+** and **-** zoom too;
   - type in **Find a task or file** to make matching cards stand out; **Enter** jumps to the first one;
   - **Legend** (or **L**), beside the zoom note, shows or hides the key to the cards and arrows; it starts hidden;
   - **Reload** (or **R**) reads the file again, after another run;
   - **Light** / **Dark** switches the theme.

## Reading the graph

- **A card** is a line in your code that sends a task: the task's name, the file and line, and a badge.
  - A teal stripe and a **start** badge mark a starting point: a task no other task sent, because it was sent from outside any task (from `main`, say), or it's a plugin's start task.
  - A blue stripe marks a task other tasks sent; its badge counts how many tasks that line made.
  - A red stripe marks where graph mode stopped, if it stopped early; its reason is in the panel.
- **An edge** is an arrow from the task that was running to the task it sent; its pill counts how many times. Busier edges are thicker and closer to teal. Edges that meet the same side of a card are spread along it.
  - An edge that skips columns is routed between the cards in the columns it crosses.
  - A loop over a card's corner is a task that sent its own kind from the same line: recursion. The layout keeps room above that card for it.
  - An edge back to an earlier column swings out underneath the cards.
- **The panel** shows an overview (the numbers, the busiest edges, the starting points), or the details of what you clicked: where a call site is, how many tasks it made and sent, which tasks sent it and which it sends. When two call sites share a name, lists add the line: `TaskD (line 83)`.

## The file it reads

The viewer keeps its own description of the export, in [`thryve_graph/src/format.rs`](thryve_graph/src/format.rs), so it doesn't depend on Thryve's crates. The file is the RON form of petgraph's `StableGraph`: a list of nodes (each a call site, with its file, line and task name), the numbers of removed nodes, and a list of edges `(from, to, count)`. Fields the viewer doesn't know are skipped, so exports from older Thryve versions (which also wrote `module_path`) and newer ones still load.

## Development

```bash
cargo test --workspace              # unit + integration tests
cargo fmt --all -- --check          # formatting
cargo clippy --workspace --all-targets -- -D warnings   # lints
```

The source is in [`thryve_graph/src`](thryve_graph/src): `format` reads the file, `model` works out the numbers, `layout` places the cards, `canvas` draws the graph and handles the mouse, `app` is the window, and `theme` holds the colours and fonts.

The background is the website's quiet version: its navy with a soft teal and blue glow, and a faint dot grid that moves and scales with the graph. Nothing runs on the GPU beyond drawing the window.

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE) (AGPL-3.0-or-later), the same license Thryve is distributed under.

## Acknowledgments

Built on [egui](https://github.com/emilk/egui) / [eframe](https://github.com/emilk/egui), [ron](https://github.com/ron-rs/ron) and [serde](https://serde.rs). Part of the [Thryve](https://github.com/thryveframework/thryve) ecosystem.
