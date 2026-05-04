//! Demo of `msg_value_bars`.
//!
//! Each circular bar owns a normalized value in [0, 1]. Click on
//! the left half of a bar to decrease the value, the right half to
//! increase it. The bars then animate to the new target via the configured
//! follow lerp.
//!
//! Press number keys 1–9 to set every bar's integer `pixel_size`. Each
//! logical pixel of the bar then renders as an N×N block of screen pixels;
//! the rest of the UI (labels, header) is unaffected.
//!
//! Run with:
//!     cargo run -p msg_value_bars --example basic

use bevy::color::palettes::css;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use msg_value_bars::prelude::*;

const STEP: f32 = 0.1;
const HIT_PADDING: f32 = 12.0;
const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "msg_value_bars — basic demo".into(),
                resolution: (1180u32, 900u32).into(),
                ..Default::default()
            }),
            ..Default::default()
        }))
        .add_plugins(ValueBarPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_pixel_size_input,
                handle_clicks.before(fan_out_staged_fills),
                fan_out_staged_fills.before(ValueBarSystems::Advance),
                update_concentric_chains
                    .after(ValueBarSystems::Advance)
                    .before(ValueBarSystems::Sync),
            ),
        )
        .run();
}

/// Bar centers are described in UI-parent-local pixels relative to the
/// centering wrapper, with Y pointing down.
#[derive(Component)]
struct DemoCenter {
    position: Vec2,
    /// Bar's outer radius in logical pixels — used to compute the click hitbox.
    hit_outer_radius: f32,
    /// Bars whose value should be modified on click.
    targets: Vec<Entity>,
}

/// Drives a stack of `growing_ring` bars so the gap between them stays
/// constant — each ring's inner radius is re-derived from its inner
/// neighbor's currently visible outer radius.
#[derive(Component)]
struct ConcentricChain {
    /// Rings in order from innermost to outermost.
    rings: Vec<Entity>,
    /// Inner radius of the innermost ring (the one with no neighbor inside).
    base_inner: f32,
    /// Per-ring radial thickness when the ring is fully filled.
    thickness: f32,
    /// Margin in logical pixels kept between adjacent rings.
    gap: f32,
}

/// Drives a stack of bars from a single master value. The master's
/// `[0, 1]` range is split into equal sub-ranges, one per stage; while a
/// stage's sub-range is being crossed, that stage is the leading band and
/// is the only one rendered with an outer outline. Earlier stages have
/// already been "covered" by the next band so their outline is hidden,
/// producing a continuous color-banded look with no seams between bands.
#[derive(Component)]
struct StagedFill {
    /// Stage bars in order from innermost / first-to-fill to outermost.
    stages: Vec<Entity>,
    /// Outer margin width (in logical pixels) restored on the leading band.
    leading_margin: f32,
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Root fills the screen. `origin_node` is a zero-sized child positioned
    // at the screen midpoint via `left/top: 50%`, so its absolute-positioned
    // descendants treat (0, 0) as the center of the window. UiScale further
    // multiplies every UI length to enlarge the layout uniformly.
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..Default::default()
        })
        .id();
    let origin_node = commands
        .spawn((
            Node {
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                ..Default::default()
            },
            ChildOf(root),
        ))
        .id();

    let cols = [-380.0, 0.0, 380.0];
    // Y-down: rows[0] is the top row.
    let rows = [-260.0, 0.0, 260.0];
    let label_offset = 130.0;

    let label_font = TextFont {
        font_size: 16.0,
        ..Default::default()
    };

    // ------------------------------------------------------------------
    // 1. Simple angular ring (270° sweep, like a power gauge).
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[0], rows[0]);
        let bar = spawn_bar(
            &mut commands,
            CircularBar::sector(58.0, 44.0, -PI * 1.25, PI * 0.25)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::LIME),
                    Color::Srgba(css::ORANGE_RED),
                    Color::Srgba(css::DARK_SLATE_GRAY),
                ),
            CircularBarValue::new(0.4).with_follow_speed(0.6),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 64.0, vec![bar]);
        spawn_label(
            &mut commands,
            "Angular fill\n(270° gauge)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 2. Full ring with angular fill (0..360°).
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[1], rows[0]);
        let bar = spawn_bar(
            &mut commands,
            CircularBar::ring(58.0, 46.0)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::AQUA),
                    Color::Srgba(css::CRIMSON),
                    Color::Srgba(css::MIDNIGHT_BLUE),
                ),
            CircularBarValue::new(0.65).with_follow_speed(0.4),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 64.0, vec![bar]);
        spawn_label(
            &mut commands,
            "Full annular ring\n(angular fill)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 3. Bounded radial fill — radial counterpart of example 4.
    //
    // FillAxis::Radius drives the fill's outer radius. Pinning the frame
    // with FrameAnchor::Full keeps the outline at the max outer radius,
    // and a non-transparent `background_color` paints the empty outer
    // band between the fill and the frame.
    //
    // Drop the value: the gold band shrinks inward, the red follow trails,
    // the dark background fills the outer gap, the outline stays at max.
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[2], rows[0]);
        let bar = spawn_bar(
            &mut commands,
            CircularBar::growing_ring(60.0, 14.0, -PI, 0.0)
                .with_origin(center)
                .with_margin(1.0)
                .with_frame_anchor(FrameAnchor::Full)
                .with_colors(
                    Color::Srgba(css::GOLD),
                    Color::Srgba(css::DARK_RED),
                    Color::Srgba(Srgba::rgb(0.10, 0.07, 0.02)),
                )
                .with_background(Color::Srgba(Srgba::rgb(0.18, 0.13, 0.04))),
            CircularBarValue::new(0.8).with_follow_speed(0.5),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 64.0, vec![bar]);
        spawn_label(
            &mut commands,
            "Bounded radial\n(fill / follow / bg / outline)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 4. Static-frame sector with a visible background color.
    //
    // FrameAnchor::Full pins the outline to the bar's max extent, so the
    // sector always reads as "0..max". Pixels inside the frame but outside
    // the lead get `background_color` — so a non-transparent background
    // shows the unfilled portion as a third color.
    //
    // Click left to drop the value: the green fill drops first, the red
    // follow trails behind, the background remains, and the outline stays
    // at the max. e.g. dropping to 80%: fill → 0.8, background → 1.0,
    // outline at 1.0.
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[0], rows[1]);
        let bar = spawn_bar(
            &mut commands,
            CircularBar::sector(58.0, 44.0, -PI * 0.75, PI * 0.75)
                .with_origin(center)
                .with_margin(1.0)
                .with_frame_anchor(FrameAnchor::Full)
                .with_colors(
                    Color::Srgba(css::SPRING_GREEN),
                    Color::Srgba(css::TOMATO),
                    Color::Srgba(css::WHITE_SMOKE),
                )
                .with_background(Color::Srgba(Srgba::rgb(0.10, 0.18, 0.10))),
            CircularBarValue::new(0.8).with_follow_speed(0.5),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 64.0, vec![bar]);
        spawn_label(
            &mut commands,
            "Bounded fill\n(fill / follow / bg / outline)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 5. Nested concentric bars driven by a shared value.
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[1], rows[1]);
        let outer = spawn_bar(
            &mut commands,
            CircularBar::ring(64.0, 56.0)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::HOT_PINK),
                    Color::Srgba(css::DARK_RED),
                    Color::Srgba(Srgba::rgb(0.05, 0.02, 0.05)),
                ),
            CircularBarValue::new(0.5).with_follow_speed(0.45),
            origin_node,
        );
        let mid = spawn_bar(
            &mut commands,
            CircularBar::growing_ring(50.0, 32.0, 0.0, TAU)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::AQUAMARINE),
                    Color::Srgba(css::ORANGE_RED),
                    Color::Srgba(Srgba::rgb(0.02, 0.05, 0.05)),
                ),
            CircularBarValue::new(0.5).with_follow_speed(0.7),
            origin_node,
        );
        let inner = spawn_bar(
            &mut commands,
            CircularBar::sector(26.0, 16.0, -PI, 0.0)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::YELLOW),
                    Color::Srgba(css::DARK_RED),
                    Color::Srgba(Srgba::rgb(0.04, 0.04, 0.0)),
                ),
            CircularBarValue::new(0.5).with_follow_speed(1.2),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 70.0, vec![outer, mid, inner]);
        spawn_label(
            &mut commands,
            "Nested concentric\n(shared value)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 6. Slow-follow gauge — exaggerated red gap to showcase the lerp.
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[2], rows[1]);
        let bar = spawn_bar(
            &mut commands,
            CircularBar::sector(60.0, 40.0, PI * 0.75, PI * 2.25)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::CHARTREUSE),
                    Color::Srgba(css::RED),
                    Color::Srgba(Srgba::rgb(0.04, 0.06, 0.02)),
                ),
            CircularBarValue::new(0.5).with_follow_speed(0.18),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 64.0, vec![bar]);
        spawn_label(
            &mut commands,
            "Slow follow\n(big red gap)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // Header / instructions.
    // ------------------------------------------------------------------
    spawn_label(
        &mut commands,
        "Left-click: decrease     Right-click: increase\nPress 1-9 to set bar pixel_size",
        Vec2::new(0.0, -380.0),
        TextFont {
            font_size: 18.0,
            ..Default::default()
        },
        origin_node,
    );

    // ------------------------------------------------------------------
    // 7. Cone-shaped radial fill — same radial fill as #3 but with the
    // angular sweep clipped to 0..30°, so the outer radius grows along
    // a thin wedge instead of a full ring.
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[0], rows[2]);
        let bar = spawn_bar(
            &mut commands,
            CircularBar::growing_ring(60.0, 14.0, 0.0, PI / 6.0)
                .with_origin(center)
                .with_margin(1.0)
                .with_colors(
                    Color::Srgba(css::GOLD),
                    Color::Srgba(css::DARK_RED),
                    Color::Srgba(Srgba::rgb(0.10, 0.07, 0.02)),
                ),
            CircularBarValue::new(0.5).with_follow_speed(0.5),
            origin_node,
        );
        spawn_demo_center(&mut commands, center, 64.0, vec![bar]);
        spawn_label(
            &mut commands,
            "Cone radial fill\n(0..30° wedge)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 8. Stacked radial fills — gap between rings stays constant by
    // re-deriving each ring's inner radius from its inner neighbor's
    // current outer radius (see `update_concentric_chains`).
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[1], rows[2]);
        const BASE_INNER: f32 = 18.0;
        const RING_THICKNESS: f32 = 16.0;
        const GAP: f32 = 4.0;
        const RING_COUNT: usize = 3;
        let outer_max =
            BASE_INNER + RING_COUNT as f32 * RING_THICKNESS + (RING_COUNT - 1) as f32 * GAP;
        let quad_size = (outer_max * 2.0 + 4.0).ceil();

        let palette = [
            (css::ORANGE, css::DARK_RED, Srgba::rgb(0.05, 0.02, 0.0)),
            (css::AQUAMARINE, css::CRIMSON, Srgba::rgb(0.0, 0.05, 0.05)),
            (css::VIOLET, css::DARK_RED, Srgba::rgb(0.05, 0.0, 0.05)),
        ];

        let mut rings = Vec::with_capacity(RING_COUNT);
        for i in 0..RING_COUNT {
            let mut bar =
                CircularBar::growing_ring(BASE_INNER + RING_THICKNESS, BASE_INNER, 0.0, TAU)
                    .with_origin(center)
                    .with_margin(1.0)
                    .with_colors(
                        Color::Srgba(palette[i].0),
                        Color::Srgba(palette[i].1),
                        Color::Srgba(palette[i].2),
                    );
            // Each ring shares the same UI quad so all three can shift
            // their inner radius outward without clipping.
            bar.quad_size = quad_size;
            rings.push(spawn_bar(
                &mut commands,
                bar,
                CircularBarValue::new(0.5).with_follow_speed(0.5 + 0.2 * i as f32),
                origin_node,
            ));
        }

        commands.spawn(ConcentricChain {
            rings: rings.clone(),
            base_inner: BASE_INNER,
            thickness: RING_THICKNESS,
            gap: GAP,
        });

        spawn_demo_center(&mut commands, center, outer_max + 6.0, rings);
        spawn_label(
            &mut commands,
            "Concentric radial\n(constant margin)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }

    // ------------------------------------------------------------------
    // 9. Color-banded stack — a single master value sweeps through three
    // stacked growing rings. Each ring drops its outer outline once the
    // next ring starts to grow, so adjacent bands meet as flat color
    // steps with no seam, and only the leading band keeps a visible edge.
    // ------------------------------------------------------------------
    {
        let center = Vec2::new(cols[2], rows[2]);
        const BAND_BASE: f32 = 4.0;
        const BAND_THICKNESS: f32 = 18.0;
        const BAND_COUNT: usize = 3;
        let outer_max = BAND_BASE + BAND_COUNT as f32 * BAND_THICKNESS;
        let quad_size = (outer_max * 2.0 + 4.0).ceil();

        let band_palette = [
            (
                css::CRIMSON,
                Srgba::rgb(0.45, 0.05, 0.05),
                Srgba::rgb(0.05, 0.0, 0.0),
            ),
            (
                css::GOLD,
                Srgba::rgb(0.45, 0.30, 0.05),
                Srgba::rgb(0.05, 0.04, 0.0),
            ),
            (
                css::LIME,
                Srgba::rgb(0.05, 0.45, 0.05),
                Srgba::rgb(0.0, 0.05, 0.0),
            ),
        ];

        let master_initial = 0.55_f32;
        let n = BAND_COUNT as f32;

        let mut stages = Vec::with_capacity(BAND_COUNT);
        for i in 0..BAND_COUNT {
            let inner = BAND_BASE + i as f32 * BAND_THICKNESS;
            let outer = inner + BAND_THICKNESS;
            let mut bar = CircularBar::growing_ring(outer, inner, 0.0, TAU)
                .with_origin(center)
                .with_colors(
                    Color::Srgba(band_palette[i].0),
                    Color::Srgba(band_palette[i].1),
                    Color::Srgba(css::WHITE),
                );
            // Strip every margin by default; the staged-fill driver
            // restores `outer_margin` only on the band currently leading.
            // No inner margin ever, so adjacent bands meet without a seam.
            bar.outer_margin = 0.0;
            bar.inner_margin = 0.0;
            bar.angular_margin = 0.0;
            bar.quad_size = quad_size;

            let t_start = i as f32 / n;
            let t_end = (i + 1) as f32 / n;
            let initial_v = ((master_initial - t_start) / (t_end - t_start)).clamp(0.0, 1.0);

            stages.push(spawn_bar(
                &mut commands,
                bar,
                CircularBarValue::new(initial_v).with_follow_speed(0.6),
                origin_node,
            ));
        }

        let master = commands
            .spawn((
                CircularBarValue::new(master_initial).with_follow_speed(0.0),
                StagedFill {
                    stages: stages.clone(),
                    leading_margin: 2.0,
                },
            ))
            .id();

        spawn_demo_center(&mut commands, center, outer_max + 6.0, vec![master]);
        spawn_label(
            &mut commands,
            "Color bands\n(only leading band keeps outline)",
            center + Vec2::new(0.0, label_offset),
            label_font.clone(),
            origin_node,
        );
    }
}

fn spawn_bar(
    commands: &mut Commands,
    bar: CircularBar,
    value: CircularBarValue,
    parent: Entity,
) -> Entity {
    commands.spawn((bar, value, ChildOf(parent))).id()
}

fn spawn_demo_center(
    commands: &mut Commands,
    position: Vec2,
    hit_outer_radius: f32,
    targets: Vec<Entity>,
) {
    commands.spawn(DemoCenter {
        position,
        hit_outer_radius,
        targets,
    });
}

fn spawn_label(
    commands: &mut Commands,
    text: &str,
    position: Vec2,
    font: TextFont,
    parent: Entity,
) {
    let half_w = 140.0;
    let half_h = 32.0;
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(position.x - half_w),
            top: Val::Px(position.y - half_h),
            width: Val::Px(half_w * 2.0),
            height: Val::Px(half_h * 2.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..Default::default()
        },
        ChildOf(parent),
        children![(
            Text::new(text),
            font,
            TextColor(Color::Srgba(css::WHITE)),
            TextLayout::new_with_justify(Justify::Center),
        )],
    ));
}

/// Re-anchor each ring in a `ConcentricChain` so its inner radius hugs the
/// outer edge of its inner neighbor with a constant `gap`. The "outer edge"
/// tracks `max(value, displayed)` of the neighbor so the hard red lead never
/// pokes through the ring sitting outside it.
fn update_concentric_chains(
    chains: Query<&ConcentricChain>,
    values: Query<&CircularBarValue>,
    mut bars: Query<&mut CircularBar>,
) {
    for chain in &chains {
        let mut current_inner = chain.base_inner;
        for &ring in &chain.rings {
            let level = values
                .get(ring)
                .map(|v| v.value.max(v.displayed).clamp(0.0, 1.0))
                .unwrap_or(0.0);
            if let Ok(mut bar) = bars.get_mut(ring) {
                bar.min_geometry.inner_radius = current_inner;
                bar.min_geometry.outer_radius = current_inner;
                bar.max_geometry.inner_radius = current_inner;
                bar.max_geometry.outer_radius = current_inner + chain.thickness;
            }
            current_inner += chain.thickness * level + chain.gap;
        }
    }
}

/// Project the master value of each `StagedFill` onto its stages: split
/// `[0, 1]` into N equal sub-ranges and write each stage's sub-range
/// fraction into its `CircularBarValue.value`. The currently-leading
/// stage gets its `outer_margin` restored; non-leading stages keep
/// `outer_margin = 0` so the boundary between stacked bands is a clean
/// color step with no inner outline drawn at all.
///
/// The leading band is picked by `displayed`, not `value`, so the outline
/// stays on whichever band's trail is currently lerping. The leading band
/// also switches to [`FrameAnchor::Fill`], anchoring the outline to the
/// trail (`min(value, displayed)`) — so the outline can never run ahead of
/// the green fill when the master jumps across a band boundary. Once the
/// trail catches up to the next band, leadership hands off seamlessly.
fn fan_out_staged_fills(
    masters: Query<(&StagedFill, &CircularBarValue)>,
    mut bars: Query<(&mut CircularBarValue, &mut CircularBar), Without<StagedFill>>,
) {
    for (fill, master) in &masters {
        if fill.stages.is_empty() {
            continue;
        }
        let n = fill.stages.len();
        let level = master.value.clamp(0.0, 1.0);
        let target_levels: Vec<f32> = (0..n)
            .map(|i| {
                let t_start = i as f32 / n as f32;
                let t_end = (i + 1) as f32 / n as f32;
                ((level - t_start) / (t_end - t_start)).clamp(0.0, 1.0)
            })
            .collect();

        // Snapshot each stage's current trail (displayed) so we can pick
        // the leading band before mutating any stage.
        let displayed_levels: Vec<f32> = fill
            .stages
            .iter()
            .map(|&s| {
                bars.get(s)
                    .map(|(v, _)| v.displayed.clamp(0.0, 1.0))
                    .unwrap_or(0.0)
            })
            .collect();

        // Innermost band whose trail still hasn't caught up to its target —
        // that's where the outline should sit. If every band is settled,
        // fall back to the outermost partially-filled band.
        const EPS: f32 = 1e-3;
        let leading_idx = (0..n)
            .find(|&i| target_levels[i] > 0.0 && displayed_levels[i] + EPS < target_levels[i])
            .or_else(|| (0..n).rev().find(|&i| target_levels[i] > 0.0))
            .unwrap_or(0);

        for (i, &stage) in fill.stages.iter().enumerate() {
            let v = target_levels[i];
            let leading = i == leading_idx;
            let filling = leading && displayed_levels[i] + EPS < v;
            if let Ok((mut bar_value, mut bar)) = bars.get_mut(stage) {
                bar_value.value = v;
                bar.outer_margin = if leading { fill.leading_margin } else { 0.0 };
                bar.frame_anchor = if filling {
                    FrameAnchor::Fill
                } else {
                    FrameAnchor::Lead
                };
            }
        }
    }
}

fn handle_pixel_size_input(keys: Res<ButtonInput<KeyCode>>, mut bars: Query<&mut CircularBar>) {
    let mappings = [
        (KeyCode::Digit1, 1_u32),
        (KeyCode::Digit2, 2),
        (KeyCode::Digit3, 3),
        (KeyCode::Digit4, 4),
        (KeyCode::Digit5, 5),
        (KeyCode::Digit6, 6),
        (KeyCode::Digit7, 7),
        (KeyCode::Digit8, 8),
        (KeyCode::Digit9, 9),
    ];
    for (key, pixel_size) in mappings {
        if keys.just_pressed(key) {
            for mut bar in &mut bars {
                bar.pixel_size = pixel_size;
            }
        }
    }
}

fn handle_clicks(
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    centers: Query<&DemoCenter>,
    bars: Query<&CircularBar>,
    mut values: Query<&mut CircularBarValue>,
) {
    let pressed_left = mouse.just_pressed(MouseButton::Left);
    let pressed_right = mouse.just_pressed(MouseButton::Right);
    if !pressed_left && !pressed_right {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // `origin_node` sits at the screen midpoint, so a bar at `position`
    // (UI-local pixels) is centered at `screen_center + position`. The bar's
    // visible radius is its logical radius times its `pixel_size`.
    let screen_center = Vec2::new(window.width(), window.height()) * 0.5;
    let delta = if pressed_right { STEP } else { -STEP };

    for center in &centers {
        let pixel_size = center
            .targets
            .iter()
            .find_map(|t| bars.get(*t).ok())
            .map(|bar| bar.pixel_size.max(1) as f32)
            .unwrap_or(1.0);
        let bar_center_screen = screen_center + center.position;
        let offset = cursor - bar_center_screen;
        if offset.length() <= center.hit_outer_radius * pixel_size + HIT_PADDING {
            for &target in &center.targets {
                if let Ok(mut value) = values.get_mut(target) {
                    value.add(delta);
                }
            }
        }
    }
}
