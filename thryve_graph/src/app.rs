//! The window: the top bar, the details panel, the graph, and the notes drawn over it.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use egui::{
    Align, Align2, Color32, CornerRadius, CursorIcon, FontFamily, FontId, Key, Layout, Margin,
    Mesh, Painter, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::{
    canvas::{GraphView, Pick, role_color},
    format::{self, LoadError},
    model::{Model, Role},
    theme::{self, Mode, Palette},
};

/// Which file the window shows, and whether it's the default one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// The export to read.
    pub path: PathBuf,
    /// No file was named when the viewer started, so it opened the default one.
    pub is_default: bool,
}

impl Source {
    /// The file named on the command line (the first argument that isn't an option), or else
    /// the default one.
    pub fn from_args(args: impl IntoIterator<Item = OsString>) -> Self {
        match args
            .into_iter()
            .find(|arg| !arg.to_string_lossy().starts_with("--"))
        {
            Some(path) => Self {
                path: PathBuf::from(path),
                is_default: false,
            },
            None => Self {
                path: Self::default_path(),
                is_default: true,
            },
        }
    }

    /// `topology\topology_export.ron` in the viewer's own folder: where Thryve's export lands
    /// when the program and the viewer share a folder and no root folder is set.
    pub fn default_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_default()
            .join("topology")
            .join("topology_export.ron")
    }

    /// The file's name, for the title bar.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.display().to_string())
    }
}

/// What the window shows: a graph, or why there isn't one.
enum Content {
    Graph(Box<GraphView>),
    Problem(LoadError),
}

fn load(source: &Source) -> Content {
    match format::load(&source.path) {
        Ok(graph) => Content::Graph(Box::new(GraphView::new(Model::new(graph)))),
        Err(error) => Content::Problem(error),
    }
}

/// What a click in the details panel asks for.
enum Action {
    Select(Pick),
    Overview,
}

/// The viewer's window.
pub struct TopologyApp {
    source: Source,
    content: Content,
    palette: Palette,
    /// The theme egui's style was last set for.
    styled: Option<Mode>,
    search: String,
    /// Whether the legend is shown. It starts hidden; the Legend button beside the zoom note,
    /// or L, switches it.
    legend: bool,
}

impl TopologyApp {
    /// Opens `source`, and sets up the fonts on `ctx`, the context the window is drawn with.
    pub fn new(ctx: &egui::Context, source: Source) -> Self {
        theme::install_fonts(ctx);
        Self {
            content: load(&source),
            source,
            palette: Palette::dark(),
            styled: None,
            search: String::new(),
            legend: false,
        }
    }

    /// Whether the legend is shown.
    pub fn legend_shown(&self) -> bool {
        self.legend
    }

    /// Reads the file again.
    pub fn reload(&mut self) {
        self.content = load(&self.source);
    }

    /// The graph, when the file loaded.
    pub fn graph(&self) -> Option<&GraphView> {
        match &self.content {
            Content::Graph(view) => Some(view),
            Content::Problem(_) => None,
        }
    }

    /// Why there's no graph, when the file didn't load.
    pub fn problem(&self) -> Option<&LoadError> {
        match &self.content {
            Content::Graph(_) => None,
            Content::Problem(error) => Some(error),
        }
    }

    /// The colours in use.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Draws the whole window into `ui`.
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.styled != Some(self.palette.mode) {
            theme::apply(&ctx, &self.palette);
            self.styled = Some(self.palette.mode);
        }
        self.keyboard(&ctx);

        paint_background(ui.painter(), ui.max_rect(), &self.palette);

        self.top_bar(ui);

        let palette = self.palette;
        if let Content::Graph(view) = &mut self.content
            && !view.is_empty()
        {
            egui::Panel::right("details")
                .frame(palette.glass().outer_margin(Margin {
                    left: 0,
                    right: 12,
                    top: 0,
                    bottom: 12,
                }))
                .resizable(true)
                .default_size(310.0)
                .size_range(260.0..=560.0)
                .show_separator_line(false)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            let action = match view.selected {
                                None => overview(ui, view, &palette),
                                Some(Pick::Site(site)) => site_details(ui, view, site, &palette),
                                Some(Pick::Edge(edge)) => edge_details(ui, view, edge, &palette),
                            };
                            match action {
                                Some(Action::Select(pick)) => {
                                    view.selected = Some(pick);
                                    view.reveal(pick);
                                }
                                Some(Action::Overview) => view.selected = None,
                                None => {}
                            }
                        });
                });
        }

        let mut reload = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                let rect = ui.max_rect();
                match &mut self.content {
                    Content::Graph(view) if !view.is_empty() => {
                        // Fitting the graph keeps the bottom clear: the legend's height when
                        // it's shown, otherwise the zoom note's row.
                        let keep_clear = 16.0
                            + if self.legend {
                                legend_height(view)
                            } else {
                                NOTE_HEIGHT
                            };
                        view.show(ui, rect, &palette, &self.search, keep_clear);
                        if self.legend {
                            paint_legend(ui.painter(), rect, view, &palette);
                        }
                        let note = paint_zoom_note(ui.painter(), rect, view, &palette);
                        if legend_button(ui, note, self.legend, &palette) {
                            self.legend = !self.legend;
                        }
                    }
                    Content::Graph(_) => {
                        reload = problem_card(
                            ui,
                            &palette,
                            "The graph is empty",
                            &self.source.path,
                            None,
                            "No task was sent while graph mode was on, so there's nothing to \
                             draw. Run the program again with work to do, then reload.",
                        );
                    }
                    Content::Problem(error) => {
                        reload = problem_card(
                            ui,
                            &palette,
                            error.title(),
                            error.path(),
                            error.detail().as_deref(),
                            error.hint(),
                        );
                    }
                }
            });
        if reload {
            self.reload();
        }
    }

    /// F fits the graph, R reloads, L shows or hides the legend, Esc goes back to the overview,
    /// + and - zoom. Ignored while typing in the search box.
    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (fit, reload, legend, back, zoom_in, zoom_out) = ctx.input(|i| {
            (
                i.key_pressed(Key::F),
                i.key_pressed(Key::R),
                i.key_pressed(Key::L),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                i.key_pressed(Key::Minus),
            )
        });
        if reload {
            self.reload();
        }
        if legend {
            self.legend = !self.legend;
        }
        if let Content::Graph(view) = &mut self.content {
            if fit {
                view.fit();
            }
            if back {
                view.selected = None;
            }
            if zoom_in {
                view.zoom_by(1.2);
            }
            if zoom_out {
                view.zoom_by(1.0 / 1.2);
            }
        }
    }

    /// The brand, the file being shown, the search box and the buttons.
    fn top_bar(&mut self, ui: &mut Ui) {
        let palette = self.palette;
        let frame = palette
            .glass()
            .inner_margin(Margin::symmetric(16, 9))
            .outer_margin(Margin {
                left: 12,
                right: 12,
                top: 10,
                bottom: 10,
            });

        egui::Panel::top("top-bar")
            .frame(frame)
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Thryve")
                            .family(theme::semibold())
                            .size(19.0)
                            .color(palette.accent),
                    );
                    ui.label(RichText::new("Topology").size(19.0).color(palette.fg));
                    ui.add_space(14.0);

                    ui.label(RichText::new(self.source.file_name()).color(palette.fg))
                        .on_hover_text(self.source.path.display().to_string());
                    if let Some(folder) = self.source.path.parent() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(folder.display().to_string())
                                    .small()
                                    .color(palette.muted),
                            )
                            .truncate(),
                        );
                    }
                    if self.source.is_default {
                        pill(ui, "Default file", palette.accent2).on_hover_text(
                            "Topology was started without naming a file, so it opened the \
                             default one: topology\\topology_export.ron in its own folder. When \
                             Thryve starts Topology at shutdown, it names the file it wrote.",
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let other = match palette.mode {
                            Mode::Dark => Mode::Light,
                            Mode::Light => Mode::Dark,
                        };
                        let label = match other {
                            Mode::Light => "Light",
                            Mode::Dark => "Dark",
                        };
                        if ui.button(label).on_hover_text("Switch the theme").clicked() {
                            self.palette = Palette::of(other);
                        }
                        if ui
                            .button("Reload")
                            .on_hover_text("Read the file again (R)")
                            .clicked()
                        {
                            self.reload();
                        }

                        if let Content::Graph(view) = &mut self.content
                            && !view.is_empty()
                        {
                            if ui
                                .button("Fit")
                                .on_hover_text("Show the whole graph (F)")
                                .clicked()
                            {
                                view.fit();
                            }

                            let search = ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .hint_text("Find a task or file")
                                    .desired_width(210.0)
                            );
                            let matches: Vec<usize> = (0..view.model.sites.len())
                                .filter(|&site| view.model.matches(site, &self.search))
                                .collect();
                            if !self.search.trim().is_empty() {
                                let note = match matches.len() {
                                    0 => "no match".to_string(),
                                    1 => "1 match".to_string(),
                                    n => format!("{n} matches"),
                                };
                                ui.label(RichText::new(note).small().color(palette.muted));
                            }
                            // Enter selects the first match and brings it into view.
                            if search.lost_focus()
                                && ui.input(|i| i.key_pressed(Key::Enter))
                                && let Some(&first) = matches.first()
                            {
                                view.selected = Some(Pick::Site(first));
                                view.reveal(Pick::Site(first));
                            }
                        }
                    });
                });
            });
    }
}

impl eframe::App for TopologyApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.bg.to_normalized_gamma_f32()
    }
}

/// The page: the website's background colour with its two soft glows, teal from the top left
/// and blue from the top right. The graph draws its dot grid over it.
fn paint_background(painter: &Painter, rect: Rect, palette: &Palette) {
    painter.rect_filled(rect, CornerRadius::ZERO, palette.bg);
    glow(
        painter,
        pos2(
            rect.left() + rect.width() * 0.15,
            rect.top() - rect.height() * 0.1,
        ),
        rect.width().max(rect.height()) * 0.7,
        palette.accent.gamma_multiply(0.11),
    );
    glow(
        painter,
        pos2(rect.right(), rect.top()),
        rect.width().max(rect.height()) * 0.55,
        palette.accent2.gamma_multiply(0.10),
    );
}

/// A soft round glow: solid at the centre, fading out by the edge.
fn glow(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    const STEPS: u32 = 48;
    let rings = [(0.0, 1.0), (0.45, 0.5), (1.0, 0.0)];
    let mut mesh = Mesh::default();
    for &(distance, strength) in &rings {
        for step in 0..STEPS {
            let angle = step as f32 / STEPS as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(
                center + vec2(angle.cos(), angle.sin()) * radius * distance,
                color.gamma_multiply(strength),
            );
        }
    }
    for ring in 0..(rings.len() as u32 - 1) {
        for step in 0..STEPS {
            let a = ring * STEPS + step;
            let b = ring * STEPS + (step + 1) % STEPS;
            let (c, d) = (a + STEPS, b + STEPS);
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// A small rounded label with a coloured outline.
fn pill(ui: &mut Ui, text: &str, color: Color32) -> egui::Response {
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.8)))
        .fill(color.gamma_multiply(0.12))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| ui.label(RichText::new(text).small().color(color)))
        .response
}

/// A section heading in the details panel.
fn heading(ui: &mut Ui, text: &str, palette: &Palette) {
    ui.add_space(10.0);
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.5)
            .color(palette.muted)
            .family(theme::semibold()),
    );
    ui.add_space(2.0);
}

/// A clickable row: a coloured dot, a name, and a number or place on the right.
fn row(ui: &mut Ui, palette: &Palette, dot: Color32, left: &str, right: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(8),
            palette.accent.gamma_multiply(0.10),
        );
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    painter.circle_filled(pos2(rect.left() + 10.0, rect.center().y), 3.5, dot);

    let right = painter.layout_no_wrap(
        right.to_string(),
        FontId::new(12.0, FontFamily::Monospace),
        palette.muted,
    );
    let right_pos = pos2(
        rect.right() - 8.0 - right.size().x,
        rect.center().y - right.size().y / 2.0,
    );
    let room = (right_pos.x - rect.left() - 32.0).max(20.0);
    let mut job = egui::text::LayoutJob::simple_singleline(
        left.to_string(),
        FontId::new(14.0, FontFamily::Proportional),
        palette.fg,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let left = painter.layout_job(job);
    painter.galley(
        pos2(rect.left() + 22.0, rect.center().y - left.size().y / 2.0),
        left,
        palette.fg,
    );
    painter.galley(right_pos, right, palette.muted);
    response.clicked()
}

/// A label and a number, side by side, for the numbers grids.
fn stat(ui: &mut Ui, palette: &Palette, label: &str, value: u64) {
    ui.label(RichText::new(label).color(palette.muted));
    ui.label(
        RichText::new(Model::count_text(value))
            .family(theme::semibold())
            .color(palette.fg),
    );
    ui.end_row();
}

/// A tinted box for something to notice.
fn callout(ui: &mut Ui, color: Color32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.10))
        .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.5)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// The details panel with nothing selected: what the graph shows, its numbers, and where to look.
fn overview(ui: &mut Ui, view: &GraphView, palette: &Palette) -> Option<Action> {
    let model = &view.model;
    let mut action = None;

    ui.label(RichText::new("Overview").heading().color(palette.fg));
    ui.label(
        RichText::new(
            "Each card is a line in your code that sends a task. Each edge is an arrow from the \
             task that was running to the task it sent; its number counts how many times.",
        )
        .color(palette.muted),
    );

    heading(ui, "Numbers", palette);
    egui::Grid::new("numbers")
        .num_columns(2)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            stat(ui, palette, "Call sites", model.sites.len() as u64);
            stat(ui, palette, "Edges", model.edges.len() as u64);
            stat(
                ui,
                palette,
                "Tasks sent from inside tasks",
                model.total_sent,
            );
            stat(ui, palette, "Starting points", model.starts().len() as u64);
        });

    if let Some(failure) = model.failure() {
        ui.add_space(10.0);
        callout(ui, palette.danger, |ui| {
            ui.label(
                RichText::new("Graph mode stopped early")
                    .family(theme::semibold())
                    .color(palette.danger),
            );
            if let Some(reason) = &model.sites[failure].failure {
                ui.label(RichText::new(reason).color(palette.fg));
            }
            ui.label(
                RichText::new(
                    "Everything recorded before then is shown. Graph mode stays off until the \
                     program restarts.",
                )
                .small()
                .color(palette.muted),
            );
            if ui.button("Show where").clicked() {
                action = Some(Action::Select(Pick::Site(failure)));
            }
        });
    }

    if !model.edges.is_empty() {
        heading(ui, "Busiest edges", palette);
        for edge in model.busiest_edges(6) {
            let ends = model.edges[edge];
            let to = if ends.to == ends.from {
                "itself"
            } else {
                model.sites[ends.to].label.as_str()
            };
            let text = format!("{} → {}", model.sites[ends.from].label, to);
            let count = format!("×{}", Model::count_text(ends.count));
            if row(ui, palette, palette.vein, &text, &count) {
                action = Some(Action::Select(Pick::Edge(edge)));
            }
        }
    }

    heading(ui, "Starting points", palette);
    for site in model.starts().into_iter().take(10) {
        let place = format!("{}:{}", model.sites[site].file_name, model.sites[site].line);
        if row(
            ui,
            palette,
            palette.accent,
            &model.sites[site].label,
            &place,
        ) {
            action = Some(Action::Select(Pick::Site(site)));
        }
    }

    ui.add_space(14.0);
    ui.label(
        RichText::new(
            "Click a card or an arrow for its details. Scroll to zoom, drag to move, and \
             double-click or press F to fit.",
        )
        .small()
        .color(palette.muted),
    );
    action
}

/// The details panel for one call site.
fn site_details(ui: &mut Ui, view: &GraphView, index: usize, palette: &Palette) -> Option<Action> {
    let model = &view.model;
    let site = &model.sites[index];
    let mut action = None;

    if ui.button("← Overview").clicked() {
        action = Some(Action::Overview);
    }
    ui.add_space(6.0);
    ui.label(RichText::new(&site.name).heading().color(palette.fg));
    let (role, explanation) = match site.role {
        Role::Start => (
            "Starting point",
            "No other call site's task sent this one: it was sent from outside any task (from \
             main, say), or it's a plugin's start task.",
        ),
        Role::Task => (
            "Task",
            "This line made the task while the tasks under \"Sent by\" were running.",
        ),
        Role::Failure => (
            "Where graph mode stopped",
            "Graph mode stopped recording here. Everything before it is shown.",
        ),
    };
    pill(ui, role, role_color(site.role, palette));
    ui.label(RichText::new(explanation).small().color(palette.muted));

    if let Some(reason) = &site.failure {
        ui.add_space(8.0);
        callout(ui, palette.danger, |ui| {
            ui.label(RichText::new(reason).color(palette.fg));
        });
        return action;
    }

    heading(ui, "Where", palette);
    ui.label(RichText::new(&site.file).monospace().color(palette.fg));
    ui.label(RichText::new(format!("line {}", site.line)).color(palette.muted));

    heading(ui, "Numbers", palette);
    egui::Grid::new("site-numbers")
        .num_columns(2)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            stat(ui, palette, "Made by other tasks", site.made_by_others);
            if site.made_by_itself > 0 {
                stat(ui, palette, "Made by its own task", site.made_by_itself);
            }
            stat(ui, palette, "Tasks it sent", site.sent_to_others);
        });

    let others_in: Vec<usize> = site
        .incoming
        .iter()
        .copied()
        .filter(|&l| model.edges[l].from != index)
        .collect();
    if !others_in.is_empty() {
        heading(ui, "Sent by", palette);
        for edge in others_in {
            let from = &model.sites[model.edges[edge].from];
            let count = format!("×{}", Model::count_text(model.edges[edge].count));
            if row(
                ui,
                palette,
                role_color(from.role, palette),
                &from.label,
                &count,
            ) {
                action = Some(Action::Select(Pick::Site(model.edges[edge].from)));
            }
        }
    }

    if !site.outgoing.is_empty() {
        heading(ui, "Sends", palette);
        for &edge in &site.outgoing {
            let ends = model.edges[edge];
            let to = &model.sites[ends.to];
            let name = if ends.to == index {
                "itself".to_string()
            } else {
                to.label.clone()
            };
            let count = format!("×{}", Model::count_text(ends.count));
            if row(ui, palette, role_color(to.role, palette), &name, &count) {
                action = Some(Action::Select(if ends.to == index {
                    Pick::Edge(edge)
                } else {
                    Pick::Site(ends.to)
                }));
            }
        }
    }

    heading(ui, "Fingerprint", palette);
    ui.label(
        RichText::new(&site.fingerprint)
            .monospace()
            .size(11.5)
            .color(palette.muted),
    );
    action
}

/// The details panel for one edge.
fn edge_details(ui: &mut Ui, view: &GraphView, index: usize, palette: &Palette) -> Option<Action> {
    let model = &view.model;
    let edge = model.edges[index];
    let (from, to) = (&model.sites[edge.from], &model.sites[edge.to]);
    let mut action = None;

    if ui.button("← Overview").clicked() {
        action = Some(Action::Overview);
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Edge").heading().color(palette.fg));
    ui.label(
        RichText::new(format!("{} → {}", from.label, to.label))
            .size(16.0)
            .color(palette.fg),
    );
    ui.label(
        RichText::new(format!("Sent {} times", Model::count_text(edge.count)))
            .family(theme::semibold())
            .size(16.0)
            .color(palette.vein),
    );
    let story = if edge.from == edge.to {
        format!(
            "A {} task sent another {} task from the same line, {} times: recursion.",
            from.name,
            to.name,
            Model::count_text(edge.count)
        )
    } else {
        format!(
            "While a {} task was running, the line at {}:{} sent a {} task, {} times.",
            from.name,
            to.file_name,
            to.line,
            to.name,
            Model::count_text(edge.count)
        )
    };
    ui.label(RichText::new(story).color(palette.muted));

    heading(ui, "From", palette);
    let place = format!("{}:{}", from.file_name, from.line);
    if row(
        ui,
        palette,
        role_color(from.role, palette),
        &from.label,
        &place,
    ) {
        action = Some(Action::Select(Pick::Site(edge.from)));
    }
    heading(ui, "To", palette);
    let place = format!("{}:{}", to.file_name, to.line);
    if row(ui, palette, role_color(to.role, palette), &to.label, &place) {
        action = Some(Action::Select(Pick::Site(edge.to)));
    }
    action
}

/// The centred card shown instead of a graph. Returns whether "Reload" was clicked.
fn problem_card(
    ui: &mut Ui,
    palette: &Palette,
    title: &str,
    path: &Path,
    detail: Option<&str>,
    hint: &str,
) -> bool {
    let mut reload = false;
    egui::Area::new(egui::Id::new("problem-card"))
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ui.ctx(), |ui| {
            palette
                .glass()
                .inner_margin(Margin::same(24))
                .show(ui, |ui| {
                    ui.set_width(520.0);
                    ui.label(RichText::new(title).heading().color(palette.fg));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(path.display().to_string())
                            .monospace()
                            .color(palette.muted),
                    );
                    if let Some(detail) = detail {
                        ui.add_space(8.0);
                        callout(ui, palette.danger, |ui| {
                            ui.label(
                                RichText::new(detail)
                                    .monospace()
                                    .size(12.0)
                                    .color(palette.fg),
                            );
                        });
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new(hint).color(palette.muted));
                    ui.add_space(14.0);
                    if ui
                        .button("Reload")
                        .on_hover_text("Read the file again (R)")
                        .clicked()
                    {
                        reload = true;
                    }
                });
        });
    reload
}

/// How tall the legend is: a row per entry, and one more when graph mode stopped early.
fn legend_height(view: &GraphView) -> f32 {
    let rows = 3 + usize::from(view.model.failure().is_some());
    rows as f32 * 20.0 + 16.0
}

/// The key in the canvas's bottom left corner: what the colours and arrows mean. Drawn only
/// while the Legend button, or L, has it on.
fn paint_legend(painter: &Painter, rect: Rect, view: &GraphView, palette: &Palette) {
    let mut rows = vec![
        (Some(palette.accent), "Starting point"),
        (
            Some(palette.accent2),
            "Task, with how many times its line ran",
        ),
        (None, "Edge: from → to, x times"),
    ];
    if view.model.failure().is_some() {
        rows.push((Some(palette.danger), "Where graph mode stopped"));
    }

    let font = FontId::new(12.0, FontFamily::Proportional);
    let galleys: Vec<_> = rows
        .iter()
        .map(|(_, text)| painter.layout_no_wrap(text.to_string(), font.clone(), palette.muted))
        .collect();
    let width = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 52.0;
    let height = legend_height(view);
    let frame = Rect::from_min_size(
        pos2(rect.left() + 16.0, rect.bottom() - 16.0 - height),
        vec2(width, height),
    );
    painter.rect(
        frame,
        CornerRadius::same(12),
        palette.surface,
        Stroke::new(1.0_f32, palette.border),
        StrokeKind::Inside,
    );

    for (i, ((swatch, _), galley)) in rows.iter().zip(galleys).enumerate() {
        let y = frame.top() + 8.0 + i as f32 * 20.0 + 10.0;
        let x = frame.left() + 12.0;
        match swatch {
            Some(color) => {
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, y - 7.0), vec2(18.0, 14.0)),
                    CornerRadius::same(3),
                    palette.card,
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, y - 7.0), vec2(4.0, 14.0)),
                    CornerRadius::same(2),
                    *color,
                );
            }
            None => {
                painter.line_segment(
                    [pos2(x, y), pos2(x + 15.0, y)],
                    Stroke::new(2.0_f32, palette.vein2.lerp_to_gamma(palette.vein, 0.5)),
                );
                painter.add(Shape::convex_polygon(
                    vec![
                        pos2(x + 19.0, y),
                        pos2(x + 13.0, y - 4.0),
                        pos2(x + 13.0, y + 4.0),
                    ],
                    palette.vein,
                    Stroke::NONE,
                ));
            }
        }
        painter.galley(
            pos2(x + 28.0, y - galley.size().y / 2.0),
            galley,
            palette.muted,
        );
    }
}

/// How tall the zoom note and the Legend button beside it are.
const NOTE_HEIGHT: f32 = 28.0;

/// The zoom level and the mouse hints, in the canvas's bottom right corner. Returns where it
/// was drawn, so that the Legend button can sit beside it.
fn paint_zoom_note(painter: &Painter, rect: Rect, view: &GraphView, palette: &Palette) -> Rect {
    let text = format!(
        "{:.0}%  ·  scroll to zoom  ·  drag to move  ·  F to fit",
        view.camera.zoom * 100.0
    );
    let galley = painter.layout_no_wrap(
        text,
        FontId::new(12.0, FontFamily::Proportional),
        palette.muted,
    );
    let size = vec2(galley.size().x + 20.0, NOTE_HEIGHT);
    let frame = Rect::from_min_size(rect.right_bottom() - vec2(16.0, 16.0) - size, size);
    painter.rect(
        frame,
        CornerRadius::same(10),
        palette.surface,
        Stroke::new(1.0_f32, palette.border),
        StrokeKind::Inside,
    );
    painter.galley(
        pos2(
            frame.left() + 10.0,
            frame.center().y - galley.size().y / 2.0,
        ),
        galley,
        palette.muted,
    );
    frame
}

/// The Legend button's id, which tests use to find it.
pub fn legend_button_id() -> egui::Id {
    egui::Id::new("legend-button")
}

/// The Legend button, just left of the zoom note at `note`: it shows or hides the legend, and
/// is lit in teal while the legend is shown. Returns whether it was clicked.
fn legend_button(ui: &mut Ui, note: Rect, shown: bool, palette: &Palette) -> bool {
    let text_color = if shown { palette.fg } else { palette.muted };
    let galley = ui.painter().layout_no_wrap(
        "Legend".to_string(),
        FontId::new(12.0, FontFamily::Proportional),
        text_color,
    );
    let size = vec2(galley.size().x + 24.0, NOTE_HEIGHT);
    let button = Rect::from_min_size(pos2(note.left() - 8.0 - size.x, note.top()), size);

    // Made after the graph, so it's on top of it: a click here never reaches the graph.
    let response = ui
        .interact(button, legend_button_id(), Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(if shown {
            "Hide the legend (L)"
        } else {
            "Show the legend (L)"
        });

    let border = if shown {
        palette.accent.gamma_multiply(0.8)
    } else if response.hovered() {
        palette.border_strong
    } else {
        palette.border
    };
    let painter = ui.painter();
    painter.rect_filled(button, CornerRadius::same(10), palette.surface);
    if shown {
        painter.rect_filled(
            button,
            CornerRadius::same(10),
            palette.accent.gamma_multiply(0.12),
        );
    }
    painter.rect_stroke(
        button,
        CornerRadius::same(10),
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
    painter.galley(
        pos2(
            button.left() + 12.0,
            button.center().y - galley.size().y / 2.0,
        ),
        galley,
        text_color,
    );
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_file_is_used_and_options_are_skipped() {
        let source = Source::from_args(["--verbose".into(), "D:\\Data\\graph.ron".into()]);

        assert_eq!(source.path, PathBuf::from("D:\\Data\\graph.ron"));
        assert!(!source.is_default);
        assert_eq!(source.file_name(), "graph.ron");
    }

    #[test]
    fn without_a_file_the_default_one_is_used_and_flagged() {
        let source = Source::from_args(Vec::<OsString>::new());

        assert!(source.is_default);
        assert!(source.path.ends_with("topology/topology_export.ron"));
    }
}
