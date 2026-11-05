//! The graph itself: the cards, the edges between them, and what the mouse does to them.
//!
//! Positions are kept in "world" units, where a card is drawn at its natural size; the
//! [`Camera`] turns them into screen positions for the current zoom and pan.

use std::collections::HashMap;

use egui::{
    Color32, CornerRadius, CursorIcon, FontFamily, FontId, Mesh, Painter, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, epaint::CubicBezierShape, pos2, vec2,
};

use crate::{
    layout,
    model::{Model, Role, Site},
    theme::{self, Palette},
};

/// The task's name on a card, at 100%.
const NAME_SIZE: f32 = 15.0;
/// The file and line on a card, at 100%.
const DETAIL_SIZE: f32 = 12.0;
/// The space inside a card.
const PAD: Vec2 = vec2(16.0, 12.0);
/// The coloured stripe on a card's left edge.
const STRIPE: f32 = 4.0;
/// A card is never narrower than this at 100%.
const MIN_WIDTH: f32 = 150.0;
/// A card is never wider than this at 100%.
const MAX_WIDTH: f32 = 340.0;
/// Names and file names longer than this are cut short with an ellipsis.
const MAX_CHARS: usize = 36;
/// How close, in screen pixels, the pointer must be to an edge to pick it.
const EDGE_REACH: f32 = 6.0;
/// The first view never zooms out further than this, so the cards stay readable; a graph too
/// big for it starts from its left, where the starting points are.
const READABLE_ZOOM: f32 = 0.45;
/// Below this zoom, a card shows only its name; below [`NAME_ZOOM`], nothing but its shape.
const DETAIL_ZOOM: f32 = 0.42;
const NAME_ZOOM: f32 = 0.24;
/// The background's dots are this far apart at 100%, in world units.
const GRID: f32 = 28.0;
/// Dots closer than this on screen are thinned out, so the grid never turns into a haze.
const GRID_MIN_GAP: f32 = 18.0;
/// The room kept above a card that sends its own kind, for its loop and the loop's count.
const LOOP_ROOM: f32 = 64.0;

/// The zoom and pan: a world position `p` is drawn at `p * zoom + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Screen pixels per world unit.
    pub zoom: f32,
    /// Where the world's origin is on the screen.
    pub offset: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: Vec2::ZERO,
        }
    }
}

impl Camera {
    /// The furthest out the view can zoom.
    pub const MIN_ZOOM: f32 = 0.15;
    /// The furthest in the view can zoom.
    pub const MAX_ZOOM: f32 = 2.5;

    /// A world position on the screen.
    pub fn to_screen(&self, world: Pos2) -> Pos2 {
        pos2(
            world.x * self.zoom + self.offset.x,
            world.y * self.zoom + self.offset.y,
        )
    }

    /// The world position under a screen position.
    pub fn to_world(&self, screen: Pos2) -> Pos2 {
        pos2(
            (screen.x - self.offset.x) / self.zoom,
            (screen.y - self.offset.y) / self.zoom,
        )
    }

    /// A world rectangle on the screen.
    pub fn rect_to_screen(&self, world: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(world.min), self.to_screen(world.max))
    }

    /// Zooms by `factor`, keeping whatever is under the screen point `anchor` where it is.
    pub fn zoom_at(&mut self, anchor: Pos2, factor: f32) {
        let world = self.to_world(anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.offset = anchor.to_vec2() - world.to_vec2() * self.zoom;
    }

    /// A camera that shows `world` inside `screen` with a margin, never closer than 100% and
    /// never further out than `min_zoom`. When `world` doesn't fit at `min_zoom`, it starts from
    /// its top left corner.
    pub fn fit(world: Rect, screen: Rect, min_zoom: f32) -> Self {
        let room = screen.shrink(48.0);
        let room = if room.is_positive() { room } else { screen };
        let fitted =
            (room.width() / world.width().max(1.0)).min(room.height() / world.height().max(1.0));
        let zoom = fitted.max(min_zoom.min(1.0)).clamp(Self::MIN_ZOOM, 1.0);

        let mut offset = screen.center().to_vec2() - world.center().to_vec2() * zoom;
        if world.width() * zoom > room.width() {
            offset.x = room.left() - world.left() * zoom;
        }
        if world.height() * zoom > room.height() {
            offset.y = room.top() - world.top() * zoom;
        }
        Self { zoom, offset }
    }
}

/// Something in the graph that can be hovered or selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// A call site's card.
    Site(usize),
    /// An edge, by its position in [`Model::edges`].
    Edge(usize),
}

/// What a drag is moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    /// One card.
    Site(usize),
    /// The whole view.
    Pan,
}

/// The graph as it's shown: where each card is, the camera, and what's hovered and selected.
pub struct GraphView {
    /// The call sites and edges.
    pub model: Model,
    /// Each card's size at 100%; measured on the first frame, when fonts are ready.
    sizes: Vec<Vec2>,
    /// Each card's centre, in world units.
    centers: Vec<Pos2>,
    /// The waypoints of edges that skip columns, in world units.
    routes: HashMap<(usize, usize), Vec<Pos2>>,
    /// The zoom and pan.
    pub camera: Camera,
    /// What the details panel shows.
    pub selected: Option<Pick>,
    /// What the pointer is on.
    pub hovered: Option<Pick>,
    /// What the current drag moves.
    drag: Option<Drag>,
    /// The camera is fitted to the graph on the next frame, zooming out no further than this.
    fit_pending: Option<f32>,
    /// Where the graph was last drawn, for centring on a call site.
    last_rect: Rect,
}

impl GraphView {
    /// A view of `model`; the cards are measured and placed on the first frame.
    pub fn new(model: Model) -> Self {
        Self {
            model,
            sizes: Vec::new(),
            centers: Vec::new(),
            routes: HashMap::new(),
            camera: Camera::default(),
            selected: None,
            hovered: None,
            drag: None,
            fit_pending: Some(READABLE_ZOOM),
            last_rect: Rect::NOTHING,
        }
    }

    /// Whether there's anything to draw.
    pub fn is_empty(&self) -> bool {
        self.model.sites.is_empty()
    }

    /// Shows the whole graph on the next frame, however far out that takes.
    pub fn fit(&mut self) {
        self.fit_pending = Some(Camera::MIN_ZOOM);
    }

    /// Zooms around the middle of the view.
    pub fn zoom_by(&mut self, factor: f32) {
        if self.last_rect.is_positive() {
            self.camera.zoom_at(self.last_rect.center(), factor);
        }
    }

    /// Moves the view so that a card, or the middle of an edge, is in the centre.
    pub fn reveal(&mut self, pick: Pick) {
        if self.centers.is_empty() || !self.last_rect.is_positive() {
            return;
        }
        let target = match pick {
            Pick::Site(site) => self.centers[site],
            Pick::Edge(edge) => {
                let edge = self.model.edges[edge];
                self.centers[edge.from].lerp(self.centers[edge.to], 0.5)
            }
        };
        if self.camera.zoom < 0.6 {
            self.camera.zoom = 0.9;
        }
        self.camera.offset =
            self.last_rect.center().to_vec2() - target.to_vec2() * self.camera.zoom;
    }

    /// The card's rectangle in world units.
    fn world_rect(&self, site: usize) -> Rect {
        Rect::from_center_size(self.centers[site], self.sizes[site])
    }

    /// The card's rectangle on the screen.
    fn screen_rect(&self, site: usize) -> Rect {
        self.camera.rect_to_screen(self.world_rect(site))
    }

    /// Everything, in world units, with room for the loops over cards.
    fn bounds(&self) -> Rect {
        (0..self.centers.len())
            .map(|site| self.world_rect(site))
            .chain(
                self.routes
                    .values()
                    .flatten()
                    .map(|&p| Rect::from_center_size(p, Vec2::ZERO)),
            )
            .reduce(|a, b| a.union(b))
            .map(|rect| rect.expand2(vec2(30.0, 60.0)))
            .unwrap_or(Rect::from_center_size(Pos2::ZERO, vec2(100.0, 100.0)))
    }

    /// Measures each card's text at 100%, then places the cards.
    fn measure(&mut self, painter: &Painter) {
        self.sizes = self
            .model
            .sites
            .iter()
            .map(|site| {
                let name = painter
                    .layout_no_wrap(shorten(&site.name), name_font(1.0), Color32::WHITE)
                    .size();
                let detail = painter
                    .layout_no_wrap(detail_text(site), detail_font(1.0), Color32::WHITE)
                    .size();
                let badge = painter
                    .layout_no_wrap(badge_text(site), badge_font(1.0), Color32::WHITE)
                    .size();
                let inner = name.x.max(detail.x + 16.0 + badge.x + 10.0);
                vec2(
                    (inner + PAD.x * 2.0 + STRIPE).clamp(MIN_WIDTH, MAX_WIDTH),
                    PAD.y * 2.0 + name.y + 5.0 + detail.y,
                )
            })
            .collect();

        let edges: Vec<(usize, usize)> = self
            .model
            .edges
            .iter()
            .map(|edge| (edge.from, edge.to))
            .collect();
        // A card that sends its own kind is placed as if it were taller, then sits at the bottom
        // of that room, so its loop never runs into the card above.
        let looped: Vec<bool> = (0..self.sizes.len())
            .map(|site| edges.contains(&(site, site)))
            .collect();
        let room: Vec<Vec2> = self
            .sizes
            .iter()
            .zip(&looped)
            .map(|(&size, &looped)| size + vec2(0.0, if looped { LOOP_ROOM } else { 0.0 }))
            .collect();
        let arrangement = layout::arrange(&room, &edges);
        self.centers = arrangement
            .centers
            .iter()
            .zip(&looped)
            .map(|(&center, &looped)| {
                center + vec2(0.0, if looped { LOOP_ROOM / 2.0 } else { 0.0 })
            })
            .collect();
        self.routes = arrangement.routes;
    }

    /// Draws the graph into `rect` and handles the mouse: the wheel zooms, a drag moves a card
    /// or the view, a click selects, a double-click on empty space fits the graph. Call sites
    /// that match `query` stand out. Fitting the graph keeps the bottom `keep_clear` points of
    /// `rect` free, for the notes drawn there.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        palette: &Palette,
        query: &str,
        keep_clear: f32,
    ) {
        let response = ui.interact(rect, ui.id().with("graph"), Sense::click_and_drag());
        if self.sizes.len() != self.model.sites.len() {
            self.measure(ui.painter());
        }
        self.last_rect = rect;
        if let Some(min_zoom) = self.fit_pending
            && rect.is_positive()
        {
            let mut room = rect;
            if rect.height() > keep_clear * 3.0 {
                room.max.y -= keep_clear;
            }
            self.camera = Camera::fit(self.bounds(), room, min_zoom);
            self.fit_pending = None;
        }

        // The wheel and a pinch zoom around the pointer.
        let pointer = response.hover_pos();
        if let Some(pointer) = pointer {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = pinch * (scroll * 0.0018).exp();
            if (factor - 1.0).abs() > 1e-4 {
                self.camera.zoom_at(pointer, factor);
            }
        }

        // What the pointer is on stays the same while something is being dragged.
        let ports = self.ports();
        if self.drag.is_none() {
            self.hovered = pointer.and_then(|pointer| self.pick(pointer, &ports));
        }

        // A drag that starts on a card moves the card; anywhere else, it moves the view.
        if response.drag_started() {
            self.drag = Some(match self.hovered {
                Some(Pick::Site(site)) => Drag::Site(site),
                _ => Drag::Pan,
            });
        }
        if response.dragged() {
            let delta = response.drag_delta();
            match self.drag {
                Some(Drag::Site(site)) => self.centers[site] += delta / self.camera.zoom,
                Some(Drag::Pan) => self.camera.offset += delta,
                None => {}
            }
        }
        if response.drag_stopped() {
            self.drag = None;
        }

        if response.clicked() {
            self.selected = self.hovered;
        }
        if response.double_clicked() && self.hovered.is_none() {
            self.fit();
            ui.ctx().request_repaint();
        }

        match (self.drag, self.hovered) {
            (Some(_), _) => ui.ctx().set_cursor_icon(CursorIcon::Grabbing),
            (None, Some(_)) => ui.ctx().set_cursor_icon(CursorIcon::PointingHand),
            (None, None) => {}
        }

        let painter = ui.painter_at(rect);
        self.paint_grid(&painter, palette);
        self.paint(&painter, palette, query);

        // A short summary follows the pointer over a card or an edge.
        if self.drag.is_none()
            && let Some(pick) = self.hovered
        {
            response.on_hover_ui_at_pointer(|ui| self.tooltip(ui, pick, palette));
        }
    }

    /// The card under the pointer, or failing that the closest edge within reach.
    fn pick(&self, pointer: Pos2, ports: &[(f32, f32)]) -> Option<Pick> {
        // Cards are drawn over edges, so they're checked first, the last drawn first.
        if let Some(site) = (0..self.model.sites.len())
            .rev()
            .find(|&site| self.screen_rect(site).contains(pointer))
        {
            return Some(Pick::Site(site));
        }

        let mut best: Option<(f32, usize)> = None;
        for (edge, &port) in ports.iter().enumerate() {
            for curve in self.edge_segments(edge, port) {
                let mut previous = curve[0];
                for step in 1..=16 {
                    let next = bezier(&curve, step as f32 / 16.0);
                    let distance = distance_to_segment(pointer, previous, next);
                    if distance <= EDGE_REACH && best.is_none_or(|(closest, _)| distance < closest)
                    {
                        best = Some((distance, edge));
                    }
                    previous = next;
                }
            }
        }
        best.map(|(_, edge)| Pick::Edge(edge))
    }

    /// What stands out: the hovered thing, or else the selected one.
    fn focus(&self) -> Option<Pick> {
        self.hovered.or(self.selected)
    }

    /// How strongly a card is drawn, from 0.25 (pushed back) to 1.
    fn site_strength(&self, site: usize, query: &str) -> f32 {
        match self.focus() {
            Some(Pick::Site(focus)) => {
                let model = &self.model;
                let connected = site == focus
                    || model.sites[focus]
                        .incoming
                        .iter()
                        .chain(&model.sites[focus].outgoing)
                        .any(|&e| model.edges[e].from == site || model.edges[e].to == site);
                if connected { 1.0 } else { 0.3 }
            }
            Some(Pick::Edge(edge)) => {
                let edge = self.model.edges[edge];
                if site == edge.from || site == edge.to {
                    1.0
                } else {
                    0.3
                }
            }
            None if !query.trim().is_empty() => {
                if self.model.matches(site, query) {
                    1.0
                } else {
                    0.25
                }
            }
            None => 1.0,
        }
    }

    /// How strongly an edge is drawn, from 0.12 (pushed back) to 1.
    fn edge_strength(&self, edge: usize, query: &str) -> f32 {
        let ends = self.model.edges[edge];
        match self.focus() {
            Some(Pick::Site(focus)) => {
                if ends.from == focus || ends.to == focus {
                    1.0
                } else {
                    0.12
                }
            }
            Some(Pick::Edge(focus)) => {
                if focus == edge {
                    1.0
                } else {
                    0.12
                }
            }
            None if !query.trim().is_empty() => {
                if self.model.matches(ends.from, query) || self.model.matches(ends.to, query) {
                    0.9
                } else {
                    0.15
                }
            }
            None => 1.0,
        }
    }

    /// The background's dot grid, which moves and scales with the graph. When zoomed out, every
    /// other dot is left out, as many times as needed to keep the dots apart.
    fn paint_grid(&self, painter: &Painter, palette: &Palette) {
        let rect = painter.clip_rect();
        let spacing = grid_spacing(self.camera.zoom);
        let top_left = self.camera.to_world(rect.min);
        let bottom_right = self.camera.to_world(rect.max);
        let color = palette.border_strong.gamma_multiply(0.55);
        let dot = vec2(2.0, 2.0);

        let mut mesh = Mesh::default();
        let mut x = (top_left.x / spacing).floor() * spacing;
        while x <= bottom_right.x {
            let mut y = (top_left.y / spacing).floor() * spacing;
            while y <= bottom_right.y {
                let center = self.camera.to_screen(pos2(x, y));
                mesh.add_colored_rect(Rect::from_center_size(center, dot), color);
                y += spacing;
            }
            x += spacing;
        }
        painter.add(Shape::mesh(mesh));
    }

    /// Draws the edges, then the cards, the ones that stand out last so they're on top.
    fn paint(&self, painter: &Painter, palette: &Palette, query: &str) {
        let ports = self.ports();
        let mut edges: Vec<(f32, usize)> = (0..self.model.edges.len())
            .map(|edge| (self.edge_strength(edge, query), edge))
            .collect();
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (strength, edge) in edges {
            self.paint_edge(painter, palette, edge, ports[edge], strength);
        }

        let mut sites: Vec<(u8, usize)> = (0..self.model.sites.len())
            .map(|site| {
                let pick = Some(Pick::Site(site));
                let layer = u8::from(self.selected == pick) + 2 * u8::from(self.hovered == pick);
                (layer, site)
            })
            .collect();
        sites.sort_by_key(|&(layer, _)| layer);
        for (_, site) in sites {
            self.paint_card(painter, palette, site, self.site_strength(site, query));
        }
    }

    /// Where each edge leaves and enters its cards, as a fraction of the card's height. Edges
    /// that share a side of a card are spread along it, in the order of where their other end
    /// is, so they don't all meet in one point.
    fn ports(&self) -> Vec<(f32, f32)> {
        let edges = &self.model.edges;
        let mut ports = vec![(0.5, 0.5); edges.len()];

        // The height an edge heads for on leaving (its first waypoint, or the card it goes to),
        // and the height it comes from on arriving (its last waypoint, or the card it left).
        let toward = |e: usize| {
            let edge = edges[e];
            match self.routes.get(&(edge.from, edge.to)) {
                Some(route) if !route.is_empty() => route[0].y,
                _ => self.centers[edge.to].y,
            }
        };
        let from = |e: usize| {
            let edge = edges[e];
            match self.routes.get(&(edge.from, edge.to)) {
                Some(route) if !route.is_empty() => route[route.len() - 1].y,
                _ => self.centers[edge.from].y,
            }
        };

        for (index, site) in self.model.sites.iter().enumerate() {
            let mut leaving: Vec<usize> = site
                .outgoing
                .iter()
                .copied()
                .filter(|&e| edges[e].to != index)
                .collect();
            leaving.sort_by(|&a, &b| toward(a).total_cmp(&toward(b)));
            for (i, &e) in leaving.iter().enumerate() {
                ports[e].0 = spread(i, leaving.len());
            }

            let mut arriving: Vec<usize> = site
                .incoming
                .iter()
                .copied()
                .filter(|&e| edges[e].from != index)
                .collect();
            arriving.sort_by(|&a, &b| from(a).total_cmp(&from(b)));
            for (i, &e) in arriving.iter().enumerate() {
                ports[e].1 = spread(i, arriving.len());
            }
        }
        ports
    }

    /// An edge's path on the screen, as cubic curves joined end to end: one for most edges, one
    /// per gap for an edge routed through waypoints. `port` says where it leaves and enters its
    /// cards (see [`GraphView::ports`]).
    fn edge_segments(&self, edge: usize, port: (f32, f32)) -> Vec<[Pos2; 4]> {
        let edge = self.model.edges[edge];
        let z = self.camera.zoom;
        let from = self.screen_rect(edge.from);
        let to = self.screen_rect(edge.to);

        // A task that sends its own kind loops over its card's top right corner.
        if edge.from == edge.to {
            let start = pos2(from.right() - 22.0 * z, from.top());
            let end = pos2(from.right() - 60.0 * z, from.top());
            return vec![[
                start,
                start + vec2(36.0 * z, -54.0 * z),
                end + vec2(-36.0 * z, -54.0 * z),
                end,
            ]];
        }

        let start = pos2(from.right(), from.top() + from.height() * port.0);
        let end = pos2(to.left(), to.top() + to.height() * port.1);
        if end.x > start.x + 8.0 * z {
            // Forwards: through any waypoints, leaving and arriving level.
            let mut points = vec![start];
            if let Some(route) = self.routes.get(&(edge.from, edge.to)) {
                points.extend(route.iter().map(|&p| self.camera.to_screen(p)));
            }
            points.push(end);
            points
                .windows(2)
                .map(|pair| {
                    let (a, b) = (pair[0], pair[1]);
                    let bend = (b.x - a.x) * 0.5;
                    [a, a + vec2(bend, 0.0), b - vec2(bend, 0.0), b]
                })
                .collect()
        } else {
            // Back to an earlier column: the edge swings out past both cards, underneath them.
            let reach = 100.0 * z;
            let drop =
                from.height().max(to.height()) * 0.5 + 54.0 * z + (start.y - end.y).abs() * 0.15;
            vec![[
                start,
                start + vec2(reach, drop),
                end + vec2(-reach, drop),
                end,
            ]]
        }
    }

    /// One edge: a soft glow, the line, the arrowhead and the count.
    fn paint_edge(
        &self,
        painter: &Painter,
        palette: &Palette,
        index: usize,
        port: (f32, f32),
        strength: f32,
    ) {
        let edge = self.model.edges[index];
        let z = self.camera.zoom;
        let segments = self.edge_segments(index, port);
        let points: Vec<Pos2> = segments.iter().flatten().copied().collect();
        if !painter
            .clip_rect()
            .intersects(Rect::from_points(&points).expand(20.0))
        {
            return;
        }

        // Busier edges are thicker and closer to teal.
        let busy = busyness(edge.count, self.model.max_count);
        let color = palette.vein2.lerp_to_gamma(palette.vein, busy);
        let width = (1.2 + 2.4 * busy) * z.sqrt().clamp(0.6, 1.4);
        for (wider, alpha) in [(5.0, 0.07), (2.6, 0.16), (1.0, 1.0)] {
            for &curve in &segments {
                painter.add(CubicBezierShape::from_points_stroke(
                    curve,
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(width * wider, color.gamma_multiply(alpha * strength)),
                ));
            }
        }

        // The arrowhead, along the last curve's final stretch.
        let last = segments[segments.len() - 1];
        let direction = (last[3] - last[2]).normalized();
        let direction = if direction.is_finite() && direction != Vec2::ZERO {
            direction
        } else {
            vec2(1.0, 0.0)
        };
        let size = (7.0 + 2.5 * busy) * z.clamp(0.5, 1.5);
        let side = vec2(-direction.y, direction.x) * size * 0.55;
        let base = last[3] - direction * size;
        painter.add(Shape::convex_polygon(
            vec![last[3], base + side, base - side],
            color.gamma_multiply(strength),
            Stroke::NONE,
        ));

        // The count, in a small pill halfway along: mid-curve, or on the middle waypoint.
        let emphasized = strength >= 0.99 && self.focus().is_some();
        if z < 0.4 && !emphasized {
            return;
        }
        let middle = &segments[segments.len() / 2];
        let halfway = bezier(middle, if segments.len() % 2 == 1 { 0.5 } else { 0.0 });
        let text_color = if emphasized {
            palette.fg
        } else {
            palette.muted
        };
        let galley = painter.layout_no_wrap(
            format!("×{}", Model::count_text(edge.count)),
            FontId::new(11.5 * z.clamp(0.75, 1.3), FontFamily::Monospace),
            text_color.gamma_multiply(strength.max(0.3)),
        );
        let pill = Rect::from_center_size(halfway, galley.size() + vec2(12.0, 5.0));
        painter.rect(
            pill,
            CornerRadius::same((pill.height() / 2.0) as u8),
            palette.card.gamma_multiply(strength.max(0.4)),
            Stroke::new(1.0_f32, color.gamma_multiply(0.55 * strength)),
            StrokeKind::Inside,
        );
        painter.galley(pill.center() - galley.size() / 2.0, galley, palette.fg);
    }

    /// One card: frosted glass, a stripe for its role, the name, the file and line, and a badge.
    fn paint_card(&self, painter: &Painter, palette: &Palette, index: usize, strength: f32) {
        let site = &self.model.sites[index];
        let z = self.camera.zoom;
        let rect = self.screen_rect(index);
        if !painter.clip_rect().intersects(rect.expand(30.0)) {
            return;
        }

        let selected = self.selected == Some(Pick::Site(index));
        let hovered = self.hovered == Some(Pick::Site(index));
        let role_color = role_color(site.role, palette);
        let radius = (10.0 * z).clamp(3.0, 14.0) as u8;

        // The selected card glows, like the website's accents.
        if selected {
            for (grow, alpha) in [(8.0, 0.08), (4.0, 0.18)] {
                painter.rect_stroke(
                    rect.expand(grow * z.min(1.0)),
                    CornerRadius::same(radius.saturating_add(4)),
                    Stroke::new(3.0_f32, palette.accent.gamma_multiply(alpha)),
                    StrokeKind::Outside,
                );
            }
        }

        let border = if selected {
            Stroke::new(1.5_f32, palette.accent)
        } else if hovered {
            Stroke::new(1.2_f32, palette.border_strong)
        } else {
            Stroke::new(1.0_f32, palette.border.gamma_multiply(strength.max(0.5)))
        };
        painter.rect(
            rect,
            CornerRadius::same(radius),
            palette.card.gamma_multiply(strength.max(0.55)),
            border,
            StrokeKind::Inside,
        );
        painter.rect_filled(
            Rect::from_min_max(
                rect.min,
                pos2(rect.left() + STRIPE * z.max(0.6), rect.bottom()),
            ),
            CornerRadius {
                nw: radius,
                sw: radius,
                ne: 0,
                se: 0,
            },
            role_color.gamma_multiply(strength),
        );

        // Far out, a card is only its shape; a little closer, its name; then all of it.
        if z < NAME_ZOOM {
            return;
        }
        let left = rect.left() + (STRIPE + PAD.x) * z;
        let name = painter.layout_no_wrap(
            shorten(&site.name),
            name_font(z),
            palette.fg.gamma_multiply(strength),
        );
        if z < DETAIL_ZOOM {
            let top = rect.center().y - name.size().y / 2.0;
            painter.galley(pos2(left, top), name, palette.fg);
            return;
        }

        let top = rect.top() + PAD.y * z;
        let name_height = name.size().y;
        painter.galley(pos2(left, top), name, palette.fg);

        let detail_top = top + name_height + 5.0 * z;
        let detail = painter.layout_no_wrap(
            detail_text(site),
            detail_font(z),
            palette.muted.gamma_multiply(strength),
        );
        painter.galley(pos2(left, detail_top), detail, palette.muted);

        let badge = painter.layout_no_wrap(
            badge_text(site),
            badge_font(z),
            role_color.gamma_multiply(strength),
        );
        let badge_rect = Rect::from_min_size(
            pos2(
                rect.right() - PAD.x * z - badge.size().x - 10.0 * z,
                detail_top - 2.0 * z,
            ),
            badge.size() + vec2(10.0 * z, 4.0 * z),
        );
        painter.rect_filled(
            badge_rect,
            CornerRadius::same((badge_rect.height() / 2.0) as u8),
            role_color.gamma_multiply(0.15 * strength),
        );
        painter.galley(badge_rect.min + vec2(5.0 * z, 2.0 * z), badge, role_color);
    }

    /// The summary next to the pointer.
    fn tooltip(&self, ui: &mut Ui, pick: Pick, palette: &Palette) {
        let model = &self.model;
        match pick {
            Pick::Site(index) => {
                let site = &model.sites[index];
                ui.label(
                    egui::RichText::new(&site.name)
                        .family(theme::semibold())
                        .color(palette.fg),
                );
                if let Some(reason) = &site.failure {
                    ui.label(egui::RichText::new(reason).color(palette.danger));
                } else {
                    ui.label(
                        egui::RichText::new(format!("{}:{}", site.file, site.line))
                            .monospace()
                            .color(palette.muted),
                    );
                    ui.label(format!(
                        "Made {} times · sent {} tasks",
                        Model::count_text(site.made()),
                        Model::count_text(site.sent_to_others)
                    ));
                }
            }
            Pick::Edge(index) => {
                let edge = model.edges[index];
                let (from, to) = (&model.sites[edge.from], &model.sites[edge.to]);
                ui.label(
                    egui::RichText::new(format!("{} → {}", from.label, to.label))
                        .family(theme::semibold())
                        .color(palette.fg),
                );
                ui.label(format!("Sent {} times", Model::count_text(edge.count)));
                ui.label(
                    egui::RichText::new(format!("by {}:{}", to.file_name, to.line))
                        .monospace()
                        .color(palette.muted),
                );
            }
        }
        ui.label(
            egui::RichText::new("Click for details")
                .small()
                .color(palette.muted),
        );
    }
}

/// The colour of a card's stripe and badge.
pub fn role_color(role: Role, palette: &Palette) -> Color32 {
    match role {
        Role::Start => palette.accent,
        Role::Task => palette.accent2,
        Role::Failure => palette.danger,
    }
}

/// Where the `i`th of `n` edges on one side of a card meets it, as a fraction of the card's
/// height: the middle for one edge, spread over the middle 60% for more.
fn spread(i: usize, n: usize) -> f32 {
    if n <= 1 {
        0.5
    } else {
        0.2 + 0.6 * i as f32 / (n - 1) as f32
    }
}

/// The grid's spacing in world units at `zoom`: [`GRID`], doubled until the dots are at least
/// [`GRID_MIN_GAP`] screen pixels apart.
fn grid_spacing(zoom: f32) -> f32 {
    let mut spacing = GRID;
    while spacing * zoom < GRID_MIN_GAP {
        spacing *= 2.0;
    }
    spacing
}

/// How busy an edge is next to the busiest one, from 0 to 1, on a log scale so an edge used
/// once still shows when another is used thousands of times.
fn busyness(count: u64, max: u64) -> f32 {
    if max <= 1 {
        return 0.35;
    }
    ((count.max(1) as f32).ln() / (max as f32).ln()).clamp(0.0, 1.0)
}

/// The point at `t` (0 to 1) along a cubic curve.
fn bezier(points: &[Pos2; 4], t: f32) -> Pos2 {
    let u = 1.0 - t;
    let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    let mut out = Vec2::ZERO;
    for (point, weight) in points.iter().zip(weights) {
        out += point.to_vec2() * weight;
    }
    out.to_pos2()
}

/// How far `point` is from the segment between `a` and `b`.
fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let along = b - a;
    let length = along.length_sq();
    if length <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(along) / length).clamp(0.0, 1.0);
    point.distance(a + along * t)
}

/// `text`, cut short with an ellipsis if it's too long for a card.
fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_CHARS {
        text.to_string()
    } else {
        let kept: String = text.chars().take(MAX_CHARS - 1).collect();
        format!("{kept}…")
    }
}

/// A card's second line: where the task was made.
fn detail_text(site: &Site) -> String {
    match site.role {
        Role::Failure => "graph mode stopped here".to_string(),
        _ => format!("{}:{}", shorten(&site.file_name), site.line),
    }
}

/// A card's badge: "start", how many tasks the line made, or "stopped".
fn badge_text(site: &Site) -> String {
    match site.role {
        Role::Start => "start".to_string(),
        Role::Task => format!("×{}", Model::count_text(site.made())),
        Role::Failure => "stopped".to_string(),
    }
}

fn name_font(zoom: f32) -> FontId {
    FontId::new(NAME_SIZE * zoom, theme::semibold())
}

fn detail_font(zoom: f32) -> FontId {
    FontId::new(DETAIL_SIZE * zoom, FontFamily::Monospace)
}

fn badge_font(zoom: f32) -> FontId {
    FontId::new(DETAIL_SIZE * zoom, FontFamily::Proportional)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_world_positions_round_trip() {
        let camera = Camera {
            zoom: 1.7,
            offset: vec2(-40.0, 25.0),
        };
        let world = pos2(123.0, -45.5);
        let back = camera.to_world(camera.to_screen(world));

        assert!((back - world).length() < 1e-3);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let mut camera = Camera::default();
        let pointer = pos2(300.0, 200.0);
        let before = camera.to_world(pointer);
        camera.zoom_at(pointer, 1.6);

        assert!((camera.to_world(pointer) - before).length() < 1e-3);
        assert!((camera.zoom - 1.6).abs() < 1e-6);
    }

    #[test]
    fn zoom_stays_within_its_limits() {
        let mut camera = Camera::default();
        camera.zoom_at(Pos2::ZERO, 1000.0);
        assert_eq!(camera.zoom, Camera::MAX_ZOOM);
        camera.zoom_at(Pos2::ZERO, 0.0001);
        assert_eq!(camera.zoom, Camera::MIN_ZOOM);
    }

    #[test]
    fn fitting_shows_everything_and_never_zooms_past_100_percent() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let small = Rect::from_min_size(pos2(500.0, 500.0), vec2(100.0, 50.0));
        let big = Rect::from_min_size(pos2(-2000.0, 0.0), vec2(6000.0, 900.0));

        assert_eq!(Camera::fit(small, screen, Camera::MIN_ZOOM).zoom, 1.0);
        let camera = Camera::fit(big, screen, Camera::MIN_ZOOM);
        assert!(screen.contains_rect(camera.rect_to_screen(big)));
    }

    #[test]
    fn a_graph_too_big_to_read_starts_from_its_left() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let big = Rect::from_min_size(pos2(-2000.0, 0.0), vec2(6000.0, 400.0));
        let camera = Camera::fit(big, screen, READABLE_ZOOM);

        assert_eq!(camera.zoom, READABLE_ZOOM);
        assert!((camera.to_screen(big.left_top()).x - 48.0).abs() < 1e-3);
    }

    #[test]
    fn edges_sharing_a_side_are_spread_along_it() {
        assert_eq!(spread(0, 1), 0.5);
        assert_eq!(spread(0, 3), 0.2);
        assert!((spread(1, 3) - 0.5).abs() < 1e-6);
        assert!((spread(2, 3) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn the_grid_thins_out_as_the_view_zooms_out() {
        assert_eq!(grid_spacing(1.0), GRID);
        assert_eq!(grid_spacing(0.5), GRID * 2.0);
        assert!(grid_spacing(0.15) * 0.15 >= GRID_MIN_GAP);
    }

    #[test]
    fn busyness_is_logarithmic_and_bounded() {
        assert_eq!(busyness(1, 1000), 0.0);
        assert_eq!(busyness(1000, 1000), 1.0);
        assert!(busyness(31, 1000) > 0.45 && busyness(31, 1000) < 0.55);
        assert_eq!(busyness(5, 1), 0.35);
    }

    #[test]
    fn a_segment_s_distance_is_measured_to_its_nearest_point() {
        let (a, b) = (pos2(0.0, 0.0), pos2(10.0, 0.0));

        assert_eq!(distance_to_segment(pos2(5.0, 3.0), a, b), 3.0);
        assert_eq!(distance_to_segment(pos2(-4.0, 3.0), a, b), 5.0);
    }

    #[test]
    fn long_names_are_cut_with_an_ellipsis() {
        let long = "x".repeat(MAX_CHARS + 10);

        assert_eq!(shorten("Hash"), "Hash");
        assert_eq!(shorten(&long).chars().count(), MAX_CHARS);
        assert!(shorten(&long).ends_with('…'));
    }
}
