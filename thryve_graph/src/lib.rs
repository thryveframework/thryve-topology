//! Topology: the viewer for the task graph Thryve writes in graph mode.
//!
//! - [`format`] reads `topology_export.ron`, with the viewer's own description of the file;
//! - [`model`] works out what the window shows: call sites, edges and their counts;
//! - [`layout`] places the call sites in columns, from the starting points to the right;
//! - [`theme`] holds the website's colours, the fonts and the panel style;
//! - [`canvas`] draws the graph and handles the mouse;
//! - [`app`] is the window: the top bar, the details panel and the canvas.

pub mod app;
pub mod canvas;
pub mod format;
pub mod layout;
pub mod model;
pub mod theme;
