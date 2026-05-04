//! Demo of `msg_value_bars` shaped as a black-hole-style HUD.
//!
//! A single master scalar drives three concentric radial bars:
//!
//! 1. Innermost wedge (0..90°): yellow outline anchored to max, black fill, orange background, red
//!    follow. Maps the master range `[0, 1]`.
//! 2. Middle wedge (0..30°): two radial color bands. The inner band (90% of the radial width) has
//!    yellow fill and fills as the master rises from `1` to `2.35`. The outer band (10% of the
//!    radial width) has pale orange fill and only starts filling once the inner band is full,
//!    covering the final slice up to `2.5`. A shared dark outline hides the seam between bands;
//!    only the leading band shows its outer edge.
//! 3. Outer banded ring (0..15°): five sub-bands stepping from orange into progressively deeper
//!    reds, fused into a single color-banded gradient by hiding every band's outline except the
//!    leading one. A static outline bar paints a fixed top, bottom, and inner edge around the
//!    whole composition while the outer (right) edge stays owned by the traveling leading-band
//!    outline. Each sub-band covers `0.5` units of master, and the outermost band has no overall
//!    maximum — past that point the master clamps at the deepest red.
//!
//! Left-click anywhere on the Status Bars to decrease the master, right-
//! click to increase. The bars animate to the new target via their follow
//! lerp. Press `1`-`9` to scale every bar's `pixel_size`; the Status Bars
//! and label auto-recenter as a flex column when the size changes.
//!
//! Run with:
//!     cargo run -p msg_value_bars --example black_hole

use bevy::color::palettes::css;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use msg_value_bars::prelude::*;

const STEP: f32 = 0.1;

const RING_GAP: f32 = 2.0;
const STAGE_1_END: f32 = 1.0;
const STAGE_2_END: f32 = 2.5;
const BAND_COUNT: usize = 5;
const BAND_WIDTH: f32 = 0.5;
const MASTER_MAX: f32 = STAGE_2_END + BAND_WIDTH * BAND_COUNT as f32;

// Adjust BAR1_SIZE to change bar 1's total radial thickness.
const BAR1_SIZE: f32 = 22.0;
const BAR1_INNER: f32 = 0.0;
const BAR1_OUTER: f32 = BAR1_INNER + BAR1_SIZE;

// Adjust BAR2_SIZE to change bar 2's total radial thickness.
// BAR2_INNER_FRAC controls where the two color bands divide (fraction of total width).
// The same fraction is used to determine when the inner band finishes filling.
const BAR2_SIZE: f32 = 22.0;
const BAR2_INNER_FRAC: f32 = 0.9;
const BAR2_INNER: f32 = BAR1_OUTER + RING_GAP;
const BAR2_OUTER: f32 = BAR2_INNER + BAR2_SIZE;
const BAR2_SPLIT: f32 = BAR2_INNER + BAR2_SIZE * BAR2_INNER_FRAC;

// Adjust BAR3_SIZE to change bar 3's total radial thickness; the five bands
// scale proportionally. BAND_GROWTH controls how much wider each successive
// band is relative to its inner neighbor.
const BAR3_SIZE: f32 = 94.0;
const BAND_GROWTH: f32 = 1.1;
const BAR3_INNER: f32 = BAR2_OUTER + RING_GAP;
const BAR3_OUTER: f32 = BAR3_INNER + BAR3_SIZE;
const BAND_THICKNESS: f32 = {
    const G1: f32 = BAND_GROWTH;
    const G2: f32 = G1 * BAND_GROWTH;
    const G3: f32 = G2 * BAND_GROWTH;
    const G4: f32 = G3 * BAND_GROWTH;
    const G5: f32 = G4 * BAND_GROWTH;
    BAR3_SIZE / (G1 + G2 + G3 + G4 + G5)
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "msg_value_bars — black hole demo".into(),
                resolution: (640u32, 720u32).into(),
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
                sync_status_bars_layout
                    .after(handle_pixel_size_input)
                    .before(ValueBarSystems::Sync),
                handle_clicks.before(drive_bars),
                drive_bars.before(ValueBarSystems::Advance),
            ),
        )
        .run();
}

#[derive(Component)]
struct StatusBars;

#[derive(Component)]
struct BlackHole {
    value: f32,
    inner: Entity,
    mid_bands: [Entity; 2],
    bands: Vec<Entity>,
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Root is a flex column that centers its children both axes. The
    // constellation and label live inside as flex items so the whole HUD
    // re-centers automatically when the constellation grows with
    // `pixel_size`.
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(24.0),
            ..Default::default()
        })
        .id();

    let quad_size = (BAR3_OUTER * 2.0 + 4.0).ceil();

    // Container for all bar nodes. `sync_status_bars_layout` resizes this
    // every frame to the tight union bounding box of all bars, so the flex
    // column always centers the exact visible area.
    let status_bars = commands
        .spawn((
            StatusBars,
            Node {
                width: Val::Px(quad_size),
                height: Val::Px(quad_size),
                flex_shrink: 0.0,
                ..Default::default()
            },
            ChildOf(root),
        ))
        .id();

    let center = Vec2::splat(quad_size * 0.5);
    let near_black = Color::Srgba(Srgba::rgb(0.04, 0.04, 0.04));

    // Each bar's end angle is chosen so that the outer arc's vertical reach
    // (outer_radius * sin(end_angle)) equals BAR1_OUTER — the radius of the
    // innermost circle. This keeps all bars at the same visual height
    // regardless of how far out they sit.
    let bar1_angle = std::f32::consts::FRAC_PI_2;
    let bar2_angle = (BAR1_OUTER / BAR2_OUTER).asin();
    let bar3_angle = (BAR1_OUTER / BAR3_OUTER).asin();

    // ------------------------------------------------------------------
    // 1. Innermost wedge — yellow outline pinned to max, black fill, red follow. The dim olive
    //    background paints the empty outer band so the yellow ring still reads when the bar is
    //    short.
    // ------------------------------------------------------------------
    let bar1 = CircularBar::growing_ring(BAR1_OUTER, BAR1_INNER, 0.0, bar1_angle)
        .with_origin(center)
        .with_margin(1.0)
        .with_frame_anchor(FrameAnchor::Full)
        .with_colors(
            near_black,
            Color::Srgba(css::DARK_GREY),
            Color::Srgba(css::YELLOW),
        )
        .with_background(Color::Srgba(css::WHITE));
    let inner = commands
        .spawn((
            bar1,
            CircularBarValue::new(0.0).with_follow_speed(0.6),
            ChildOf(status_bars),
        ))
        .id();

    // ------------------------------------------------------------------
    // 2. Middle wedge — two radial color bands. Inner 90% fills yellow, outer 10% fills pale orange
    //    once the inner band is full. Seam between bands is hidden; only the leading band shows its
    //    outer edge, so the composition reads as a single contiguous bar.
    // ------------------------------------------------------------------
    let bar2_bg = Color::Srgba(Srgba::rgb(0.06, 0.05, 0.0));
    let pale_orange = Srgba::rgb(1.0, 0.80, 0.52);

    let mut bar2_inner = CircularBar::growing_ring(BAR2_SPLIT, BAR2_INNER, 0.0, bar2_angle)
        .with_origin(center)
        .with_colors(
            Color::Srgba(css::YELLOW),
            Color::from(Srgba::rgb(1.00, 0.55, 0.10)),
            near_black,
        )
        .with_background(bar2_bg);
    bar2_inner.outer_margin = 0.0;
    bar2_inner.inner_margin = 1.0;
    bar2_inner.angular_margin = 1.0;
    let mid_inner = commands
        .spawn((
            bar2_inner,
            CircularBarValue::new(0.0).with_follow_speed(0.6),
            ChildOf(status_bars),
        ))
        .id();

    let mut bar2_outer = CircularBar::growing_ring(BAR2_OUTER, BAR2_SPLIT, 0.0, bar2_angle)
        .with_origin(center)
        .with_colors(
            Color::Srgba(pale_orange),
            Color::from(Srgba::rgb(1.00, 0.55, 0.10)),
            near_black,
        )
        .with_background(bar2_bg);
    bar2_outer.outer_margin = 0.0;
    bar2_outer.inner_margin = 0.0;
    bar2_outer.angular_margin = 1.0;
    let mid_outer = commands
        .spawn((
            bar2_outer,
            CircularBarValue::new(0.0).with_follow_speed(0.6),
            ChildOf(status_bars),
        ))
        .id();

    // ------------------------------------------------------------------
    // 3. Outer banded ring — four sub-bands stepping from orange into progressively deeper reds.
    //    Inner-band radial seams are stripped so adjacent bands meet without a black gap; each band
    //    still paints its own angular (top/bottom) outline via `angular_margin`, and the innermost
    //    band paints the inner-radius (left) outline via `inner_margin`. Because every band uses
    //    `FrameAnchor::Lead`, those outlines naturally only appear within the band's filled extent
    //    — empty bands have no outline at all. `drive_bars` keeps the leading band's `outer_margin`
    //    on so the right edge of the composition is the traveling outline.
    // ------------------------------------------------------------------
    let band_palette = [
        Srgba::rgb(1.00, 0.55, 0.10), // orange
        Srgba::rgb(0.90, 0.25, 0.05), // deep orange-red
        Srgba::rgb(0.65, 0.08, 0.05), // crimson
        Srgba::rgb(0.32, 0.03, 0.03), // dark red
        Srgba::rgb(0.15, 0.01, 0.01), // near-black red
    ];
    let band_follow = Srgba::rgb(0.18, 0.0, 0.0);
    let mut bands = Vec::with_capacity(BAND_COUNT);
    let mut current_inner = BAR3_INNER;
    for (i, &band_color) in band_palette.iter().enumerate() {
        let inner_r = current_inner;
        let outer_r = inner_r + BAND_THICKNESS * BAND_GROWTH.powi(i as i32 + 1);
        current_inner = outer_r;
        let mut band = CircularBar::growing_ring(outer_r, inner_r, 0.0, bar3_angle)
            .with_origin(center)
            .with_colors(
                Color::Srgba(band_color),
                Color::Srgba(band_follow),
                near_black,
            );
        band.outer_margin = 0.0;
        // Only the innermost band carries the left (inner-radius) edge of the
        // composition. Inner-radius seams between adjacent bands stay hidden so
        // the bands flow into each other.
        band.inner_margin = if i == 0 { 1.0 } else { 0.0 };
        // Every band paints its own top/bottom edges; stacked, they form the
        // continuous angular outline of the composition. With `FrameAnchor::Lead`
        // these only paint within the band's lead extent, so empty bands stay
        // outline-free.
        band.angular_margin = 1.0;
        bands.push(
            commands
                .spawn((
                    band,
                    CircularBarValue::new(0.0).with_follow_speed(0.6),
                    ChildOf(status_bars),
                ))
                .id(),
        );
    }

    commands.spawn(BlackHole {
        value: 0.0,
        inner,
        mid_bands: [mid_inner, mid_outer],
        bands,
    });

    commands.spawn((
        Node {
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..Default::default()
        },
        ChildOf(root),
        children![(
            Text::new("Black hole intensity\nLeft-click: decrease    Right-click: increase\nPress 1-9 to set bar pixel_size"),
            TextFont { font_size: 16.0, ..Default::default() },
            TextColor(Color::Srgba(css::WHITE)),
            TextLayout::new_with_justify(Justify::Center),
        )],
    ));
}

/// Resizes the Status Bars node to the tight union of all bars' bounding
/// boxes and re-anchors every bar's origin so the center of the circular
/// diagram stays in the center of that tight container. Runs every frame so
/// digit-key changes to `pixel_size` are picked up immediately.
fn sync_status_bars_layout(
    mut status_bars: Query<&mut Node, With<StatusBars>>,
    mut bars: Query<&mut CircularBar>,
) {
    let pixel_size = bars.iter().map(|b| b.pixel_size.max(1)).max().unwrap_or(1) as f32;

    // Union of all bars' tight bounding boxes in center-relative logical pixels.
    let mut union_min = Vec2::splat(f32::MAX);
    let mut union_max = Vec2::splat(f32::MIN);
    for bar in bars.iter() {
        let (bb_min_raw, bb_max_raw) = bar.max_geometry.bounding_box(1.0);
        union_min = union_min.min(bb_min_raw.floor());
        union_max = union_max.max(bb_max_raw.ceil());
    }
    if union_min.x == f32::MAX {
        return;
    }

    let tight_size = (union_max - union_min) * pixel_size;
    if let Ok(mut node) = status_bars.single_mut() {
        node.width = Val::Px(tight_size.x);
        node.height = Val::Px(tight_size.y);
    }

    // origin = bar-center position in parent screen pixels.
    // Setting it to -union_min * pixel_size means the tight box starts at (0,0).
    let target_origin = -union_min * pixel_size;
    for mut bar in &mut bars {
        if bar.origin != target_origin {
            bar.origin = target_origin;
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

fn handle_clicks(mouse: Res<ButtonInput<MouseButton>>, mut masters: Query<&mut BlackHole>) {
    let pressed_left = mouse.pressed(MouseButton::Left);
    let pressed_right = mouse.pressed(MouseButton::Right);
    if !pressed_left && !pressed_right {
        return;
    }
    let delta = if pressed_right { STEP } else { -STEP };
    for mut master in &mut masters {
        master.value = (master.value + delta).clamp(0.0, MASTER_MAX);
    }
}

/// Project the master scalar onto each bar:
///
/// * `inner` covers `[0, STAGE_1_END]`.
/// * `mid`   covers `[STAGE_1_END, STAGE_2_END]`.
/// * Each band of the outer ring covers a `BAND_WIDTH` slice past `STAGE_2_END`. The band that is
///   partially filled (started but the next band hasn't) is "leading" and gets its outer outline
///   restored.
fn drive_bars(
    masters: Query<&BlackHole>,
    mut bars: Query<(&mut CircularBarValue, &mut CircularBar)>,
) {
    for master in &masters {
        let v = master.value;

        let bar1_v = (v / STAGE_1_END).clamp(0.0, 1.0);
        if let Ok((mut bv, _)) = bars.get_mut(master.inner) {
            bv.set(bar1_v);
        }

        let bar2_v = ((v - STAGE_1_END) / (STAGE_2_END - STAGE_1_END)).clamp(0.0, 1.0);
        let bar2_inner_v = (bar2_v / BAR2_INNER_FRAC).clamp(0.0, 1.0);
        let bar2_outer_v = ((bar2_v - BAR2_INNER_FRAC) / (1.0 - BAR2_INNER_FRAC)).clamp(0.0, 1.0);
        if let Ok((mut bv, mut bar)) = bars.get_mut(master.mid_bands[0]) {
            bv.set(bar2_inner_v);
            bar.outer_margin = if bar2_inner_v > 0.0 && bar2_outer_v == 0.0 {
                1.0
            } else {
                0.0
            };
        }
        if let Ok((mut bv, mut bar)) = bars.get_mut(master.mid_bands[1]) {
            bv.set(bar2_outer_v);
            bar.outer_margin = if bar2_outer_v > 0.0 { 1.0 } else { 0.0 };
        }

        let mut levels = Vec::with_capacity(master.bands.len());
        for i in 0..master.bands.len() {
            let t_start = STAGE_2_END + i as f32 * BAND_WIDTH;
            let t_end = t_start + BAND_WIDTH;
            levels.push(((v - t_start) / (t_end - t_start)).clamp(0.0, 1.0));
        }
        for (i, &band) in master.bands.iter().enumerate() {
            let lvl = levels[i];
            let next_lvl = levels.get(i + 1).copied().unwrap_or(0.0);
            let leading = lvl > 0.0 && next_lvl == 0.0;
            if let Ok((mut bv, mut bar)) = bars.get_mut(band) {
                bv.set(lvl);
                bar.outer_margin = if leading { 1.0 } else { 0.0 };
            }
        }
    }
}
