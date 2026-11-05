//! Real exports, end to end: reading the file, working out the model, and drawing the window.
//!
//! - `graph_test_export.ron` is what today's Thryve wrote for TestLib's graph test: a plugin's
//!   start task, a chain of tasks, two call sites that send the same task, and recursion
//!   (TaskD sends itself 110 times).
//! - `sample_topology.ron` is in the older format, which also wrote `module_path`; it must
//!   still load.

use std::path::PathBuf;

use thryve_graph::{
    app::{Source, TopologyApp, legend_button_id},
    format,
    model::{Model, Role},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn site(model: &Model, name: &str, line: u32) -> usize {
    model
        .sites
        .iter()
        .position(|site| site.name == name && site.line == line)
        .unwrap_or_else(|| panic!("no {name} at line {line}"))
}

#[test]
fn an_export_from_today_s_thryve_loads_with_its_counts() {
    let model = Model::new(format::load(&fixture("graph_test_export.ron")).unwrap());

    assert_eq!(model.sites.len(), 9);
    assert_eq!(model.edges.len(), 11);

    let start = site(&model, "TestGraph", 200);
    assert_eq!(model.sites[start].role, Role::Start);
    assert_eq!(model.starts(), vec![start]);

    // TaskD at line 107 is sent by three other call sites, 5 times each, and by itself.
    let recursive = site(&model, "TaskD", 107);
    assert_eq!(model.sites[recursive].made_by_others, 15);
    assert_eq!(model.sites[recursive].made_by_itself, 110);
    assert_eq!(model.sites[site(&model, "TaskC", 67)].sent_to_others, 15);
}

#[test]
fn an_export_from_before_module_path_was_dropped_still_loads() {
    let model = Model::new(format::load(&fixture("sample_topology.ron")).unwrap());

    assert_eq!(model.sites.len(), 4);
    assert_eq!(model.edges.len(), 2);
    assert_eq!(model.sites[site(&model, "ChildTaskA", 20)].made(), 3);
    assert_eq!(
        model.sites[site(&model, "OrphanTask", 40)].role,
        Role::Start
    );
}

/// Draws a few frames of the window, the way eframe would, without opening one. `ctx` must be
/// the context the app was made with: the app installs its fonts there.
fn draw_frames(ctx: &egui::Context, app: &mut TopologyApp, frames: usize) {
    for _ in 0..frames {
        draw_frame_with(ctx, app, Vec::new());
    }
}

/// Draws one frame of the window, with `events` (key presses, mouse moves and clicks) as its
/// input.
fn draw_frame_with(ctx: &egui::Context, app: &mut TopologyApp, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1320.0, 820.0),
        )),
        events,
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| app.draw(ui));
}

/// A key going down and back up.
fn tap(key: egui::Key) -> Vec<egui::Event> {
    [true, false]
        .into_iter()
        .map(|pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        })
        .collect()
}

/// The left mouse button going down, or back up, at `pos`.
fn mouse_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn open_test_export(ctx: &egui::Context) -> TopologyApp {
    let source = Source {
        path: fixture("graph_test_export.ron"),
        is_default: false,
    };
    TopologyApp::new(ctx, source)
}

#[test]
fn the_legend_starts_hidden_and_l_shows_and_hides_it() {
    let ctx = egui::Context::default();
    let mut app = open_test_export(&ctx);
    draw_frames(&ctx, &mut app, 2);
    assert!(!app.legend_shown());

    draw_frame_with(&ctx, &mut app, tap(egui::Key::L));
    assert!(app.legend_shown());

    draw_frame_with(&ctx, &mut app, tap(egui::Key::L));
    assert!(!app.legend_shown());
}

#[test]
fn a_click_on_the_legend_button_shows_the_legend() {
    let ctx = egui::Context::default();
    let mut app = open_test_export(&ctx);
    draw_frames(&ctx, &mut app, 2);

    let button = ctx
        .read_response(legend_button_id())
        .expect("the Legend button should be drawn beside the zoom note")
        .rect;
    let at = button.center();
    draw_frame_with(&ctx, &mut app, vec![egui::Event::PointerMoved(at)]);
    draw_frame_with(&ctx, &mut app, vec![mouse_button(at, true)]);
    draw_frame_with(&ctx, &mut app, vec![mouse_button(at, false)]);

    assert!(app.legend_shown());
}

#[test]
fn the_window_draws_a_graph_and_fits_it_on_screen() {
    let source = Source {
        path: fixture("graph_test_export.ron"),
        is_default: false,
    };
    let ctx = egui::Context::default();
    let mut app = TopologyApp::new(&ctx, source);
    draw_frames(&ctx, &mut app, 3);

    let view = app.graph().expect("the graph should load");
    assert!(view.camera.zoom > 0.0 && view.camera.zoom <= 1.0);
    assert!(app.problem().is_none());
}

#[test]
fn the_window_explains_a_missing_file_instead_of_showing_an_empty_canvas() {
    let source = Source {
        path: fixture("no_such_export.ron"),
        is_default: true,
    };
    let ctx = egui::Context::default();
    let mut app = TopologyApp::new(&ctx, source);
    draw_frames(&ctx, &mut app, 2);

    let problem = app.problem().expect("a missing file is a problem to show");
    assert_eq!(problem.title(), "No graph here yet");
    assert!(app.graph().is_none());
}
