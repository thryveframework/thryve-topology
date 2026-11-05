# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - Unreleased

The first release of Topology, which shows the task graph that Thryve exports in graph mode.

### Added

- Opens the file named on the command line, which is how Thryve starts it at shutdown; without
  one, it opens `topology/topology_export.ron` in its own folder and says so in the window.
- Reads Thryve's export with its own description of the format, so it depends on none of
  Thryve's crates. It reads today's exports and older ones (which also wrote `module_path`), and
  skips fields it doesn't know.
- Lays the graph out in columns from the starting points, with room between them; edges that
  skip columns are routed between the cards they pass, edges that share a side of a card are
  spread along it, a card that sends its own kind gets room for its loop, and each separate
  part of the graph gets its own band.
- Cards show the task's name, its file and line, and how many tasks that line made; arrows show
  their counts, and are thicker and closer to teal the busier they are.
- A details panel: an overview (the numbers, the busiest edges, the starting points), each call
  site's place, numbers and edges, and each edge's story; names shared by several call sites get
  their line.
- Search by task name or file, a zoom note, Fit, Reload and theme buttons, and keyboard shortcuts
  (F, R, L, Esc, + and -).
- A legend, hidden at start: the Legend button beside the zoom note, or L, shows and hides it.
- The node where graph mode stopped is shown in red, with its reason.
- A missing, empty or unreadable file is explained in the window, with a Reload button, instead
  of an empty canvas.
- The look of Thryve's website: navy, teal and blue, frosted panels and soft glows, with a faint
  dot grid that moves with the graph, in a dark and a light theme.
- Opens maximized, with a window icon; a release build on Windows opens no console window behind
  it.
- Prebuilt archives for Windows (x86_64), Linux (x86_64) and macOS (Intel and Apple Silicon),
  attached to each GitHub release.
- CI: formatting, lints and tests on Windows, Linux and macOS.

[0.1.0]: https://github.com/thryveframework/thryve-topology/releases/tag/v0.1.0
