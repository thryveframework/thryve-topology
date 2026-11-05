// A release build on Windows opens only its window, without a console behind it: Thryve starts
// the viewer on its own at shutdown, and anything worth saying is said in the window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! `Topology [file]`: shows the task graph in `file`, or, without one, in
//! `topology/topology_export.ron` next to the viewer.

use std::sync::Arc;

use thryve_graph::app::{Source, TopologyApp};

fn main() -> eframe::Result<()> {
    let source = Source::from_args(std::env::args_os().skip(1));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 820.0])
            .with_min_inner_size([760.0, 480.0])
            .with_maximized(true)
            .with_title(format!("{} — Thryve Topology", source.file_name()))
            .with_icon(Arc::new(icon())),
        ..Default::default()
    };

    eframe::run_native(
        "Thryve Topology",
        options,
        Box::new(move |cc| Ok(Box::new(TopologyApp::new(&cc.egui_ctx, source)))),
    )
}

/// The window's icon, drawn here so the viewer needs no image files: on a navy tile, a teal
/// starting point with edges to two blue tasks.
fn icon() -> egui::IconData {
    const SIZE: usize = 64;
    let navy = [4.0, 7.0, 15.0];
    let teal = [34.0, 224.0, 208.0];
    let blue = [58.0, 155.0, 255.0];
    let start = (17.0, 32.0);
    let tasks = [(47.0, 17.0), (47.0, 47.0)];

    let distance_to_line = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((p.0 - a.0 - dx * t).powi(2) + (p.1 - a.1 - dy * t).powi(2)).sqrt()
    };
    let distance =
        |p: (f32, f32), c: (f32, f32)| ((p.0 - c.0).powi(2) + (p.1 - c.1).powi(2)).sqrt();

    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = (x as f32 + 0.5, y as f32 + 0.5);

            // The tile: a rounded square, softened by a pixel at its edge.
            let corner = 14.0;
            let qx = (p.0 - 32.0).abs() - (32.0 - corner);
            let qy = (p.1 - 32.0).abs() - (32.0 - corner);
            let outside =
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - corner;
            let tile = (0.5 - outside).clamp(0.0, 1.0);

            // The edges, shading from teal to blue, then the three dots on top.
            let mut color = navy;
            let edge = tasks
                .iter()
                .map(|&task| distance_to_line(p, start, task))
                .fold(f32::MAX, f32::min);
            let blend = ((p.0 - start.0) / (tasks[0].0 - start.0)).clamp(0.0, 1.0);
            let edge_color = [0, 1, 2].map(|i| teal[i] + (blue[i] - teal[i]) * blend);
            let over = |color: [f32; 3], top: [f32; 3], amount: f32| {
                [0, 1, 2].map(|i| color[i] + (top[i] - color[i]) * amount.clamp(0.0, 1.0))
            };
            color = over(color, edge_color, 2.6 - edge);
            color = over(color, teal, 7.5 - distance(p, start));
            for &task in &tasks {
                color = over(color, blue, 6.5 - distance(p, task));
            }

            rgba.extend(color.map(|c| c as u8));
            rgba.push((tile * 255.0) as u8);
        }
    }

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
