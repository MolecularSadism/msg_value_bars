//! Pixel-perfect circular value bars for Bevy.
//!
//! Each bar is a circular ring sector defined by an inner radius, outer
//! radius, start angle, and end angle — measured in *logical pixels* relative
//! to a shared origin. Bars are rendered with hard, integer-snapped pixel
//! tests so the result stays sharp under any integer up-scaling.
//!
//! Three concentric extents drive the look:
//!
//! * `frame`  — the bar's outline (the full visible region).
//! * `lead`   — the instantaneous "real" value (the red boundary).
//! * `fill`   — the lagging green portion that lerps toward `lead`.
//!
//! Driving the bar with [`CircularBarValue`] gives you the classic HUD
//! "filling plus 100" effect for free: the frame and lead jump immediately
//! to the new target, while the green fill lerps in to cover the red gap.
//!
//! # Example
//!
//! ```
//! use bevy::prelude::*;
//! use msg_value_bars::prelude::*;
//!
//! // The components compose like any other Bevy bundle. Add
//! // `ValueBarPlugin` to your full `App` so the renderer animates them.
//! let bar = CircularBar::ring(32.0, 22.0).with_margin(1.0);
//! let value = CircularBarValue::new(0.5).with_follow_speed(2.0);
//!
//! let mut app = App::new();
//! app.add_plugins(MinimalPlugins);
//! app.world_mut().spawn((bar, value));
//! ```

mod material;

use bevy::prelude::*;

pub use material::{ValueBarMaterial, ValueBarUniforms};

pub mod prelude {
    pub use super::{
        BarGeometry, CircularBar, CircularBarValue, FillAxis, FrameAnchor, ValueBarPlugin,
        ValueBarSystems,
    };
}

/// System set labels for the systems run by [`ValueBarPlugin`] in the
/// `Update` schedule. Exposing these lets callers insert their own logic
/// at well-defined points (e.g. recompute geometry between `Advance` and
/// `Sync`).
#[derive(SystemSet, Clone, Hash, PartialEq, Eq, Debug)]
pub enum ValueBarSystems {
    /// Spawns the UI node + material for newly-added bars.
    Spawn,
    /// Lerps `displayed` toward `value`.
    Advance,
    /// Pushes the latest geometry to the bar's material uniforms.
    Sync,
}

/// Plugin registering the [`ValueBarMaterial`] and the systems that drive
/// [`CircularBar`] entities.
pub struct ValueBarPlugin;

impl Plugin for ValueBarPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<UiMaterialPlugin<ValueBarMaterial>>() {
            app.add_plugins(material::plugin);
        }
        app.register_type::<CircularBar>()
            .register_type::<CircularBarValue>()
            .register_type::<BarGeometry>()
            .register_type::<FillAxis>()
            .register_type::<FrameAnchor>();

        app.add_systems(
            Update,
            (
                spawn_bar_renderer.in_set(ValueBarSystems::Spawn),
                advance_bar_value.in_set(ValueBarSystems::Advance),
                sync_bar_material.in_set(ValueBarSystems::Sync),
            )
                .chain(),
        );
    }
}

// ----------------------------------------------------------------------------
// Geometry primitive
// ----------------------------------------------------------------------------

/// A single ring-sector geometry, measured in logical pixels relative to the
/// bar's origin. Used both as the bar's full extent (`max_geometry`) and the
/// minimum extent (`min_geometry`) for value-driven interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct BarGeometry {
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub start_angle: f32,
    pub end_angle: f32,
}

impl BarGeometry {
    pub const fn new(
        inner_radius: f32,
        outer_radius: f32,
        start_angle: f32,
        end_angle: f32,
    ) -> Self {
        Self {
            inner_radius,
            outer_radius,
            start_angle,
            end_angle,
        }
    }

    /// A full ring spanning a complete revolution.
    pub const fn full_ring(inner_radius: f32, outer_radius: f32) -> Self {
        Self::new(inner_radius, outer_radius, 0.0, std::f32::consts::TAU)
    }

    /// A sector with zero sweep at the same start angle as `self`. Useful as a
    /// "min" geometry for angular fills.
    pub fn collapsed_to_start(self) -> Self {
        Self {
            end_angle: self.start_angle,
            ..self
        }
    }

    /// A sector with the outer radius collapsed to the inner radius. Useful
    /// as a "min" geometry for radial fills.
    pub fn collapsed_to_inner(self) -> Self {
        Self {
            outer_radius: self.inner_radius,
            ..self
        }
    }

    /// Compute the tight axis-aligned bounding box in center-relative logical
    /// pixels. `padding` expands every edge outward by that many pixels.
    pub fn bounding_box(&self, padding: f32) -> (Vec2, Vec2) {
        let sweep = self.end_angle - self.start_angle;
        let r = self.outer_radius;

        if sweep <= 0.0 {
            return (Vec2::ZERO, Vec2::ZERO);
        }
        if sweep >= std::f32::consts::TAU {
            return (Vec2::splat(-r - padding), Vec2::splat(r + padding));
        }

        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        let mut include = |x: f32, y: f32| {
            min.x = min.x.min(x);
            min.y = min.y.min(y);
            max.x = max.x.max(x);
            max.y = max.y.max(y);
        };

        include(r * self.start_angle.cos(), r * self.start_angle.sin());
        include(r * self.end_angle.cos(), r * self.end_angle.sin());
        let ri = self.inner_radius;
        include(ri * self.start_angle.cos(), ri * self.start_angle.sin());
        include(ri * self.end_angle.cos(), ri * self.end_angle.sin());
        if ri <= 0.0 {
            include(0.0, 0.0);
        }

        let tau = std::f32::consts::TAU;
        for k in 0..4_i32 {
            let axis = k as f32 * std::f32::consts::FRAC_PI_2;
            let a = axis - self.start_angle;
            let from_start = a - (a / tau).floor() * tau;
            if from_start <= sweep {
                include(r * axis.cos(), r * axis.sin());
            }
        }

        (
            Vec2::new(min.x - padding, min.y - padding),
            Vec2::new(max.x + padding, max.y + padding),
        )
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            inner_radius: self.inner_radius.lerp(other.inner_radius, t),
            outer_radius: self.outer_radius.lerp(other.outer_radius, t),
            start_angle: self.start_angle.lerp(other.start_angle, t),
            end_angle: self.end_angle.lerp(other.end_angle, t),
        }
    }
}

// ----------------------------------------------------------------------------
// Public component API
// ----------------------------------------------------------------------------

/// Which axis the value drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum FillAxis {
    /// The end angle interpolates between min and max.
    #[default]
    Angle,
    /// The outer radius interpolates between min and max.
    Radius,
}

/// What the frame outline tracks.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Default)]
pub enum FrameAnchor {
    /// Outline hugs whichever of `value`/`displayed` is larger. On a
    /// filling bar the frame jumps to the new target; on a depleting bar
    /// the frame ghosts down with the lerping fill.
    #[default]
    Lead,
    /// Outline hugs whichever of `value`/`displayed` is smaller — useful
    /// when you want the frame to track the green region tightly.
    Fill,
    /// Outline is fixed at the bar's full extent. Useful for static rings
    /// where only the green/red regions inside should grow.
    Full,
}

/// A circular value bar — a ring sector that reflects a 0..1 value.
///
/// Spawn alongside a [`CircularBarValue`] and the plugin will manage the mesh,
/// material, and per-frame lerp.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CircularBar {
    /// Bar center in logical pixels relative to the UI parent's top-left
    /// (Y-down). The plugin places a square node of side [`quad_size`]
    /// centered on this point.
    pub origin: Vec2,
    /// Node side length in logical pixels. Must comfortably enclose the bar's
    /// outer radius from the center; the plugin assumes the center sits at
    /// the node's midpoint.
    pub quad_size: f32,
    /// Integer scale factor applied to the rendered node size. With
    /// `pixel_size = N`, the bar takes `quad_size * N` screen pixels per side
    /// while the shader still operates on a `quad_size`-wide logical-pixel
    /// grid — so each logical pixel renders as an N×N block of screen pixels.
    pub pixel_size: u32,

    /// Bar geometry at value = 0.
    pub min_geometry: BarGeometry,
    /// Bar geometry at value = 1.
    pub max_geometry: BarGeometry,

    /// Margin width in pixels for the outer edge of the frame outline.
    /// Set to 0 to hide that edge — useful for stacking bars into a
    /// continuous color band where only the leading band shows the outline.
    pub outer_margin: f32,
    /// Margin width in pixels for the inner edge of the frame outline.
    /// Set to 0 to hide it.
    pub inner_margin: f32,
    /// Margin width in pixels for the angular (start/end) edges of the
    /// frame outline on partial sectors. Ignored on full rings. Set to
    /// 0 to hide them.
    pub angular_margin: f32,

    pub fill_color: Color,
    pub follow_color: Color,
    pub frame_color: Color,
    pub background_color: Color,

    pub fill_axis: FillAxis,
    pub frame_anchor: FrameAnchor,
}

impl Default for CircularBar {
    fn default() -> Self {
        let max = BarGeometry::full_ring(20.0, 30.0);
        Self {
            origin: Vec2::ZERO,
            quad_size: 64.0,
            pixel_size: 1,
            min_geometry: max.collapsed_to_start(),
            max_geometry: max,
            outer_margin: 1.0,
            inner_margin: 1.0,
            angular_margin: 1.0,
            fill_color: Color::srgb(0.30, 0.85, 0.35),
            follow_color: Color::srgb(0.85, 0.20, 0.20),
            frame_color: Color::srgb(0.05, 0.05, 0.07),
            background_color: Color::NONE,
            fill_axis: FillAxis::Angle,
            frame_anchor: FrameAnchor::Lead,
        }
    }
}

impl CircularBar {
    /// A full annular ring with angular fill. The bar starts empty and fills
    /// counter-clockwise as the value rises.
    pub fn ring(outer_radius: f32, inner_radius: f32) -> Self {
        let max = BarGeometry::full_ring(inner_radius, outer_radius);
        Self {
            min_geometry: max.collapsed_to_start(),
            max_geometry: max,
            quad_size: (outer_radius * 2.0 + 2.0).ceil(),
            fill_axis: FillAxis::Angle,
            ..Default::default()
        }
    }

    /// An angular sector (e.g. a HUD gauge). The frame's end angle is driven
    /// by the value.
    pub fn sector(outer_radius: f32, inner_radius: f32, start_angle: f32, end_angle: f32) -> Self {
        let max = BarGeometry::new(inner_radius, outer_radius, start_angle, end_angle);
        Self {
            min_geometry: max.collapsed_to_start(),
            max_geometry: max,
            quad_size: (outer_radius * 2.0 + 2.0).ceil(),
            fill_axis: FillAxis::Angle,
            ..Default::default()
        }
    }

    /// A bar where the outer radius grows with value. The angles stay fixed.
    pub fn growing_ring(
        max_outer_radius: f32,
        inner_radius: f32,
        start_angle: f32,
        end_angle: f32,
    ) -> Self {
        let max = BarGeometry::new(inner_radius, max_outer_radius, start_angle, end_angle);
        Self {
            min_geometry: max.collapsed_to_inner(),
            max_geometry: max,
            quad_size: (max_outer_radius * 2.0 + 2.0).ceil(),
            fill_axis: FillAxis::Radius,
            ..Default::default()
        }
    }

    pub fn with_origin(mut self, origin: Vec2) -> Self {
        self.origin = origin;
        self
    }

    /// Set the integer pixel scale (default `1`). Each logical pixel is
    /// rendered as an N×N block of screen pixels.
    pub fn with_pixel_size(mut self, pixel_size: u32) -> Self {
        self.pixel_size = pixel_size.max(1);
        self
    }

    /// Set the same margin width on every edge of the frame outline.
    pub fn with_margin(mut self, margin: f32) -> Self {
        self.outer_margin = margin;
        self.inner_margin = margin;
        self.angular_margin = margin;
        self
    }

    /// Set the margin width for the outer edge of the frame outline only.
    pub fn with_outer_margin(mut self, margin: f32) -> Self {
        self.outer_margin = margin;
        self
    }

    /// Set the margin width for the inner edge of the frame outline only.
    pub fn with_inner_margin(mut self, margin: f32) -> Self {
        self.inner_margin = margin;
        self
    }

    /// Set the margin width for the angular (start/end) edges of the frame
    /// outline only. Ignored on full rings.
    pub fn with_angular_margin(mut self, margin: f32) -> Self {
        self.angular_margin = margin;
        self
    }

    pub fn with_colors(mut self, fill: Color, follow: Color, frame: Color) -> Self {
        self.fill_color = fill;
        self.follow_color = follow;
        self.frame_color = frame;
        self
    }

    pub fn with_background(mut self, background: Color) -> Self {
        self.background_color = background;
        self
    }

    pub fn with_frame_anchor(mut self, anchor: FrameAnchor) -> Self {
        self.frame_anchor = anchor;
        self
    }

    pub fn with_min_geometry(mut self, min: BarGeometry) -> Self {
        self.min_geometry = min;
        self
    }

    /// Compute the geometry corresponding to a normalized value in [0, 1].
    pub fn geometry_at(&self, value: f32) -> BarGeometry {
        let t = value.clamp(0.0, 1.0);
        match self.fill_axis {
            FillAxis::Angle => {
                // Only the end angle should move; everything else snaps to max.
                BarGeometry {
                    inner_radius: self.max_geometry.inner_radius,
                    outer_radius: self.max_geometry.outer_radius,
                    start_angle: self.max_geometry.start_angle,
                    end_angle: self
                        .min_geometry
                        .end_angle
                        .lerp(self.max_geometry.end_angle, t),
                }
            }
            FillAxis::Radius => BarGeometry {
                inner_radius: self.max_geometry.inner_radius,
                outer_radius: self
                    .min_geometry
                    .outer_radius
                    .lerp(self.max_geometry.outer_radius, t),
                start_angle: self.max_geometry.start_angle,
                end_angle: self.max_geometry.end_angle,
            },
        }
    }
}

/// Drives the [`CircularBar`] over time.
///
/// `value` is the instantaneous target. `displayed` lerps toward `value` at
/// `follow_speed` (in normalized units per second). The frame's red boundary
/// tracks `value`; the green fill tracks `displayed`.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CircularBarValue {
    pub value: f32,
    pub displayed: f32,
    /// Lerp rate in value-units per second. 0 means instant (no follow).
    pub follow_speed: f32,
}

impl Default for CircularBarValue {
    fn default() -> Self {
        Self {
            value: 0.0,
            displayed: 0.0,
            follow_speed: 2.0,
        }
    }
}

impl CircularBarValue {
    pub fn new(value: f32) -> Self {
        let v = value.clamp(0.0, 1.0);
        Self {
            value: v,
            displayed: v,
            follow_speed: 2.0,
        }
    }

    pub fn with_follow_speed(mut self, speed: f32) -> Self {
        self.follow_speed = speed;
        self
    }

    /// Set the target value, leaving `displayed` to lerp on its own.
    pub fn set(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    /// Increment the target by `delta`, clamped to [0, 1].
    pub fn add(&mut self, delta: f32) {
        self.set(self.value + delta);
    }
}

// ----------------------------------------------------------------------------
// Internals
// ----------------------------------------------------------------------------

#[derive(Component)]
struct BarRendererSpawned;

fn spawn_bar_renderer(
    mut commands: Commands,
    mut materials: ResMut<Assets<ValueBarMaterial>>,
    bars: Query<(Entity, &CircularBar, Option<&CircularBarValue>), Without<BarRendererSpawned>>,
) {
    for (entity, bar, value) in &bars {
        let value = value.copied().unwrap_or_default();
        let material = materials.add(ValueBarMaterial {
            uniforms: build_uniforms(bar, &value),
        });

        let (bb_min_raw, bb_max_raw) = bar.max_geometry.bounding_box(1.0);
        let bb_min = bb_min_raw.floor();
        let bb_max = bb_max_raw.ceil();
        let ps = bar.pixel_size.max(1) as f32;
        commands.entity(entity).insert((
            BarRendererSpawned,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px((bb_max.x - bb_min.x) * ps),
                height: Val::Px((bb_max.y - bb_min.y) * ps),
                left: Val::Px(bar.origin.x + bb_min.x * ps),
                top: Val::Px(bar.origin.y + bb_min.y * ps),
                ..Default::default()
            },
            MaterialNode(material),
        ));
    }
}

fn advance_bar_value(time: Res<Time>, mut bars: Query<&mut CircularBarValue>) {
    let dt = time.delta_secs();
    for mut v in &mut bars {
        if v.follow_speed <= 0.0 {
            v.displayed = v.value;
            continue;
        }
        let max_step = v.follow_speed * dt;
        let diff = v.value - v.displayed;
        if diff.abs() <= max_step {
            v.displayed = v.value;
        } else {
            v.displayed += diff.signum() * max_step;
        }
    }
}

fn sync_bar_material(
    mut materials: ResMut<Assets<ValueBarMaterial>>,
    mut bars: Query<(
        &CircularBar,
        &CircularBarValue,
        &MaterialNode<ValueBarMaterial>,
        &mut Node,
    )>,
) {
    for (bar, value, material_handle, mut node) in &mut bars {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.uniforms = build_uniforms(bar, value);
        }
        let (bb_min_raw, bb_max_raw) = bar.max_geometry.bounding_box(1.0);
        let bb_min = bb_min_raw.floor();
        let bb_max = bb_max_raw.ceil();
        let ps = bar.pixel_size.max(1) as f32;
        node.width = Val::Px((bb_max.x - bb_min.x) * ps);
        node.height = Val::Px((bb_max.y - bb_min.y) * ps);
        node.left = Val::Px(bar.origin.x + bb_min.x * ps);
        node.top = Val::Px(bar.origin.y + bb_min.y * ps);
    }
}

fn color_to_vec4(c: Color) -> Vec4 {
    let lin = c.to_linear();
    Vec4::new(lin.red, lin.green, lin.blue, lin.alpha)
}

fn build_uniforms(bar: &CircularBar, value: &CircularBarValue) -> ValueBarUniforms {
    // The red "lead" is always the larger of the two values, and the green
    // "fill" the smaller. This makes the bar visually symmetric: it works the
    // same on the way up (fill chasing lead with red ahead) and on the way
    // down (fill catching up with red trailing the ghost).
    let lead_t = value.value.max(value.displayed);
    let fill_t = value.value.min(value.displayed);
    let lead = bar.geometry_at(lead_t);
    let fill = bar.geometry_at(fill_t);
    let frame = match bar.frame_anchor {
        FrameAnchor::Lead => lead,
        FrameAnchor::Fill => fill,
        FrameAnchor::Full => bar.max_geometry,
    };

    let (bb_min_raw, bb_max_raw) = bar.max_geometry.bounding_box(1.0);
    let bb_min = bb_min_raw.floor();
    let bb_max = bb_max_raw.ceil();
    let quad_px_size = bb_max - bb_min;
    let center = Vec2::new(-bb_min.x, -bb_min.y);

    ValueBarUniforms {
        quad_px_size,
        center_px: center,
        frame_outer_radius: frame.outer_radius,
        frame_inner_radius: frame.inner_radius,
        frame_start_angle: frame.start_angle,
        frame_end_angle: frame.end_angle,
        lead_outer_radius: lead.outer_radius,
        lead_inner_radius: lead.inner_radius,
        lead_start_angle: lead.start_angle,
        lead_end_angle: lead.end_angle,
        fill_outer_radius: fill.outer_radius,
        fill_inner_radius: fill.inner_radius,
        fill_start_angle: fill.start_angle,
        fill_end_angle: fill.end_angle,
        frame_margin_outer_px: bar.outer_margin,
        frame_margin_inner_px: bar.inner_margin,
        frame_margin_angular_px: bar.angular_margin,
        _pad0: 0.0,
        fill_color: color_to_vec4(bar.fill_color),
        follow_color: color_to_vec4(bar.follow_color),
        frame_color: color_to_vec4(bar.frame_color),
        background_color: color_to_vec4(bar.background_color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    #[test]
    fn angular_fill_lerps_only_end_angle() {
        let bar = CircularBar::sector(30.0, 20.0, 0.0, std::f32::consts::FRAC_PI_2);
        let g = bar.geometry_at(0.5);
        approx(g.inner_radius, 20.0);
        approx(g.outer_radius, 30.0);
        approx(g.start_angle, 0.0);
        approx(g.end_angle, std::f32::consts::FRAC_PI_2 * 0.5);
    }

    #[test]
    fn radial_fill_lerps_only_outer_radius() {
        let bar = CircularBar::growing_ring(40.0, 10.0, 0.0, std::f32::consts::TAU);
        let g_zero = bar.geometry_at(0.0);
        approx(g_zero.outer_radius, 10.0);
        let g_half = bar.geometry_at(0.5);
        approx(g_half.outer_radius, 25.0);
        let g_full = bar.geometry_at(1.0);
        approx(g_full.outer_radius, 40.0);
    }

    #[test]
    fn follow_speed_advances_displayed_toward_value() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, advance_bar_value);

        let entity = app
            .world_mut()
            .spawn(CircularBarValue {
                value: 1.0,
                displayed: 0.0,
                follow_speed: 1.0,
            })
            .id();

        // First update has dt=0, but second registers a positive delta.
        app.update();
        app.update();
        let v = app
            .world()
            .entity(entity)
            .get::<CircularBarValue>()
            .unwrap();
        assert!(v.displayed > 0.0);
        assert!(v.displayed < 1.0);
    }

    #[test]
    fn instant_follow_when_speed_is_zero() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, advance_bar_value);

        let entity = app
            .world_mut()
            .spawn(CircularBarValue {
                value: 0.7,
                displayed: 0.0,
                follow_speed: 0.0,
            })
            .id();

        app.update();
        let v = app
            .world()
            .entity(entity)
            .get::<CircularBarValue>()
            .unwrap();
        approx(v.displayed, 0.7);
    }
}
