//! Reading `topology_export.ron`, the file Thryve writes when a program in graph mode shuts down.
//!
//! The viewer keeps its own description of the file, so it doesn't depend on Thryve's crates. The
//! file is the RON form of petgraph's `StableGraph`:
//!
//! ```text
//! (
//!     nodes: [
//!         (
//!             callsite: (
//!                 file_path: "src/pipeline.rs",
//!                 line_number: 47,
//!                 callsite_name: "LoadImg",
//!                 this_callsite_fingerprint: "14348963799517950713",
//!                 current_executing_task_fingerprint: "12492297762336498906",
//!             ),
//!             fingerprint: "14348963799517950713",
//!             failure: Some("..."),      // only on the node where graph mode stopped
//!         ),
//!     ],
//!     node_holes: [],                    // numbers of removed nodes
//!     edge_property: directed,
//!     edges: [
//!         Some((0, 1, 5)),               // (from, to, how many times)
//!         None,                          // a removed edge
//!     ],
//! )
//! ```
//!
//! Each node is a call site: the line in the dev's code that made a task. An edge goes from the
//! call site of the task that was running to the call site of the task it sent, and counts how
//! many times that happened. Fields the viewer doesn't know are skipped, so files from older
//! Thryve versions (which also wrote `module_path`) and from newer ones still load.

use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// One call site, as the file describes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CallSite {
    /// The source file: relative to the dev's workspace when Thryve could tell, absolute otherwise.
    pub file_path: String,
    /// The line that made the task.
    pub line_number: u32,
    /// The task's name.
    pub callsite_name: String,
}

/// One node of the file: a call site, its fingerprint and, on the node where graph mode stopped,
/// the reason it stopped.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Node {
    /// Where the task was made.
    pub callsite: CallSite,
    /// The hash Thryve finds the node by.
    #[serde(default)]
    pub fingerprint: String,
    /// Why graph mode stopped recording; set only on the node it added when it stopped.
    #[serde(default)]
    pub failure: Option<String>,
}

/// An edge between two call sites: a task from `from` was running when `to`'s line sent a task,
/// `count` times. Both are positions in [`Graph::nodes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// The call site of the task that was running.
    pub from: usize,
    /// The call site of the task it sent.
    pub to: usize,
    /// How many times that happened.
    pub count: u64,
}

/// The whole file, with removed nodes and edges left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    /// The call sites, in the file's order.
    pub nodes: Vec<Node>,
    /// The edges, in the file's order.
    pub edges: Vec<Edge>,
}

/// Why a file couldn't be shown.
#[derive(Debug)]
pub enum LoadError {
    /// There's no file at the path.
    Missing(PathBuf),
    /// The file is there, but couldn't be read.
    Read(PathBuf, io::Error),
    /// The file has nothing in it.
    Empty(PathBuf),
    /// The file isn't a graph this viewer can read; the text says where and why.
    Format(PathBuf, String),
}

impl LoadError {
    /// The file the error is about.
    pub fn path(&self) -> &Path {
        match self {
            Self::Missing(path)
            | Self::Read(path, _)
            | Self::Empty(path)
            | Self::Format(path, _) => path,
        }
    }

    /// A short title for the window.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Missing(_) => "No graph here yet",
            Self::Read(..) => "The file couldn't be read",
            Self::Empty(_) => "The file is empty",
            Self::Format(..) => "This file isn't a graph this viewer can read",
        }
    }

    /// The details: the system's error, or the parser's message with its line and column.
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Missing(_) | Self::Empty(_) => None,
            Self::Read(_, error) => Some(error.to_string()),
            Self::Format(_, message) => Some(message.clone()),
        }
    }

    /// What the dev can do about it.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Missing(_) => {
                "Thryve writes this file when a program in graph mode shuts down: turn on \
                 `is_debugging` and `visual_graph` in the config's [debugging] section, run the \
                 program, and stop it."
            }
            Self::Read(..) => "Check that the file exists and that this account can read it.",
            Self::Empty(_) => {
                "Thryve may still be writing it, or the run was cut short. Reload once the \
                 program has shut down."
            }
            Self::Format(..) => {
                "It may come from a newer Thryve than this viewer. Use the Topology that came \
                 with your Thryve."
            }
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.title(), self.path().display())?;
        if let Some(detail) = self.detail() {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoadError {}

/// The file's top level. `edge_property` isn't read: Thryve's graph is always directed.
#[derive(Deserialize)]
struct FileGraph {
    nodes: Vec<Node>,
    #[serde(default)]
    node_holes: Vec<usize>,
    #[serde(default)]
    edges: Vec<Option<(usize, usize, u64)>>,
}

/// Reads and parses the file at `path`.
pub fn load(path: &Path) -> Result<Graph, LoadError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(LoadError::Missing(path.to_path_buf()));
        }
        Err(error) => return Err(LoadError::Read(path.to_path_buf(), error)),
    };

    if text.trim().is_empty() {
        return Err(LoadError::Empty(path.to_path_buf()));
    }

    parse(&text).map_err(|message| LoadError::Format(path.to_path_buf(), message))
}

/// Parses the text of an export. The error is the parser's message, or names the edge that
/// points to a node the file doesn't have.
pub fn parse(text: &str) -> Result<Graph, String> {
    let text = text.trim_start_matches('\u{feff}');
    let file: FileGraph = ron::from_str(text).map_err(|error| error.to_string())?;

    // The file numbers its nodes with the removed ones still counted: each hole takes a number,
    // and the nodes fill the other numbers in order.
    let total = file.nodes.len() + file.node_holes.len();
    let mut is_hole = vec![false; total];
    for &hole in &file.node_holes {
        if hole >= total || is_hole[hole] {
            return Err(format!(
                "node_holes lists {hole}, which isn't a free node number"
            ));
        }
        is_hole[hole] = true;
    }

    let mut position = vec![None; total];
    let mut next = 0;
    for (number, hole) in is_hole.iter().enumerate() {
        if !hole {
            position[number] = Some(next);
            next += 1;
        }
    }

    // Edges name nodes by those numbers; each one is turned into a position in `nodes`.
    let find = |number: usize| position.get(number).copied().flatten();
    let mut edges = Vec::new();
    for (index, edge) in file.edges.iter().enumerate() {
        let Some((from, to, count)) = *edge else {
            continue;
        };

        match (find(from), find(to)) {
            (Some(from), Some(to)) => edges.push(Edge { from, to, count }),
            _ => {
                return Err(format!(
                    "edge {index} edges node {from} to node {to}, but the file has no such node"
                ));
            }
        }
    }

    Ok(Graph {
        nodes: file.nodes,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, line: u32) -> String {
        format!(
            "(callsite: (file_path: \"src/app.rs\", line_number: {line}, callsite_name: \
             \"{name}\", this_callsite_fingerprint: \"\", current_executing_task_fingerprint: \
             \"\"), fingerprint: \"fp-{name}\")"
        )
    }

    #[test]
    fn a_file_without_holes_keeps_its_numbering() {
        let text = format!(
            "(nodes: [{}, {}], node_holes: [], edge_property: directed, edges: [Some((0, 1, 5))])",
            node("Start", 1),
            node("Hash", 2)
        );
        let graph = parse(&text).unwrap();

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.nodes[1].callsite.callsite_name, "Hash");
        assert_eq!(
            graph.edges,
            vec![Edge {
                from: 0,
                to: 1,
                count: 5
            }]
        );
    }

    #[test]
    fn holes_take_a_number_and_removed_edges_are_skipped() {
        // Numbers 0 and 2 are holes, so the file's nodes are numbers 1, 3 and 4.
        let text = format!(
            "(nodes: [{}, {}, {}], node_holes: [0, 2], edge_property: directed, \
             edges: [Some((1, 3, 2)), None, Some((3, 4, 7))])",
            node("Start", 1),
            node("Hash", 2),
            node("Save", 3)
        );
        let graph = parse(&text).unwrap();

        assert_eq!(
            graph.edges,
            vec![
                Edge {
                    from: 0,
                    to: 1,
                    count: 2
                },
                Edge {
                    from: 1,
                    to: 2,
                    count: 7
                },
            ]
        );
    }

    #[test]
    fn an_edge_to_a_hole_is_an_error_that_names_it() {
        let text = format!(
            "(nodes: [{}], node_holes: [1], edge_property: directed, edges: [Some((0, 1, 1))])",
            node("Start", 1)
        );
        let error = parse(&text).unwrap_err();

        assert!(error.contains("edge 0 edges node 0 to node 1"), "{error}");
    }

    #[test]
    fn the_failure_node_keeps_its_reason() {
        let text = "(nodes: [(callsite: (file_path: \"\", line_number: 0, callsite_name: \
                    \"Graph mode stopped\"), fingerprint: \"failure\", failure: Some(\"the \
                    topology lock was poisoned\"))], node_holes: [], edge_property: directed, \
                    edges: [])";
        let graph = parse(text).unwrap();

        assert_eq!(
            graph.nodes[0].failure.as_deref(),
            Some("the topology lock was poisoned")
        );
    }

    #[test]
    fn unknown_fields_from_older_and_newer_files_are_skipped() {
        let text = "(nodes: [(callsite: (file_path: \"src/a.rs\", module_path: \"app::a\", \
                    line_number: 3, callsite_name: \"Hash\", kind: Then), fingerprint: \"x\", \
                    count: 12)], node_holes: [], edge_property: directed, edges: [], version: 2)";

        assert_eq!(parse(text).unwrap().nodes[0].callsite.callsite_name, "Hash");
    }

    #[test]
    fn text_that_isnt_a_graph_reports_where() {
        let error = parse("(nodes: [ oops ]").unwrap_err();

        assert!(
            error.contains(':'),
            "the message should give a position: {error}"
        );
    }

    #[test]
    fn load_tells_a_missing_file_from_an_empty_one() {
        let dir = std::env::temp_dir().join(format!("thryve_graph_format_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("empty.ron");
        fs::write(&empty, "  \n").unwrap();

        assert!(matches!(
            load(&dir.join("missing.ron")),
            Err(LoadError::Missing(_))
        ));
        assert!(matches!(load(&empty), Err(LoadError::Empty(_))));

        let _ = fs::remove_dir_all(&dir);
    }
}
