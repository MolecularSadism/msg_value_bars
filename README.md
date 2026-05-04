# msg_value_bars

Pixel-perfect circular value bars (rings and sectors) for [Bevy](https://bevyengine.org/) with smooth fill/follow lerp animations.

Each bar is a ring sector rendered entirely in a WGSL fragment shader using hard, integer-snapped pixel tests — so it stays sharp under any integer up-scaling and works correctly for pixel-art UIs.

## Features

- **Rings and sectors** — full annular rings, partial arc gauges, or radially-growing wedges
- **Fill/follow animation** — the classic HUD effect: the value boundary jumps immediately while the fill color lerps to catch up, creating a visible red "debt" gap
- **Pixel-perfect rendering** — integer-snapped pixel tests keep every edge sharp at any integer pixel scale
- **Per-edge margins** — independent outline widths on the outer, inner, and angular edges
- **Configurable anchoring** — the frame outline can track the lead value, the fill, or stay fixed at full extent
- **Composable** — multiple `CircularBar` entities can be layered concentrically to build complex multi-band HUDs
- **Bevy Reflect** — all public types derive `Reflect` and are registered for the inspector and hot reloading

## Bevy Compatibility

| `msg_value_bars` | Bevy |
|------------------|------|
| 0.1              | 0.18 |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
msg_value_bars = { git = "https://github.com/MolecularSadism/msg_value_bars", tag = "v0.1.0" }
```

Then add the plugin to your app:

```rust
use msg_value_bars::prelude::*;

app.add_plugins(ValueBarPlugin);
```

## Quick Start

Spawn a `CircularBar` + `CircularBarValue` pair anywhere in a Bevy UI hierarchy. The plugin manages the `Node`, shader material, and per-frame animation automatically.

```rust
use bevy::prelude::*;
use msg_value_bars::prelude::*;

fn spawn_health_bar(mut commands: Commands) {
    commands.spawn((
        CircularBar::ring(32.0, 22.0)
            .with_margin(1.0)
            .with_origin(Vec2::new(48.0, 48.0)),
        CircularBarValue::new(1.0).with_follow_speed(2.0),
    ));
}

fn update_health(mut bars: Query<&mut CircularBarValue>, health: Res<PlayerHealth>) {
    for mut bar in &mut bars {
        bar.set(health.fraction());
    }
}
```

The green fill follows the red lead at `follow_speed` normalized units per second. Setting `follow_speed` to `0.0` makes the bar snap instantly.

## Visual Model

Three concentric extents make up each bar:

```
┌──────────────────────────────────────────┐
│  frame  — the outline (dark border)      │
│  ┌────────────────────────────────────┐  │
│  │  lead  — current value  (red)      │  │
│  │  ┌──────────────────────────────┐  │  │
│  │  │  fill — displayed  (green)   │  │  │
│  │  └──────────────────────────────┘  │  │
│  └────────────────────────────────────┘  │
└──────────────────────────────────────────┘
```

- **frame** tracks whichever extent is specified by `FrameAnchor` (default: the lead value)
- **lead** is `max(value, displayed)` — the furthest-ahead boundary, colored with `follow_color`
- **fill** is `min(value, displayed)` — the lagging region, colored with `fill_color`

This makes the animation symmetric: on the way up the red gap trails behind the green fill; on the way down the red gap ghosts ahead as the fill catches up.

## API Reference

### `ValueBarPlugin`

The main plugin. Add it once to your `App`. Registers `ValueBarMaterial` and three systems in the `Update` schedule, ordered by `ValueBarSystems`.

```rust
app.add_plugins(ValueBarPlugin);
```

### `CircularBar`

The visual configuration component. Spawn it (with or without a `CircularBarValue`) and the plugin creates the Bevy UI node and shader material.

**Factory constructors:**

| Constructor | Description |
|---|---|
| `CircularBar::ring(outer, inner)` | Full annular ring, fills counter-clockwise by angle |
| `CircularBar::sector(outer, inner, start, end)` | Arc gauge, fills from start to end angle |
| `CircularBar::growing_ring(max_outer, inner, start, end)` | Radial fill — outer radius grows with value |

Angles are in radians, measured counter-clockwise from the positive X axis (east).

**Builder methods:**

```rust
CircularBar::ring(32.0, 22.0)
    .with_origin(Vec2::new(50.0, 50.0))  // center in parent-relative logical px (Y-down)
    .with_pixel_size(2)                   // render at 2×2 screen px per logical px
    .with_margin(1.0)                     // 1 px outline on all edges
    .with_outer_margin(0.0)              // hide the outer edge only
    .with_inner_margin(1.0)              // inner edge outline
    .with_angular_margin(1.0)            // start/end edge outlines (sectors only)
    .with_colors(fill, follow, frame)    // set fill, lead, and frame colors
    .with_background(Color::NONE)        // color behind everything (transparent by default)
    .with_frame_anchor(FrameAnchor::Full) // fix frame at full extent
    .with_min_geometry(custom_min)       // override the value=0 geometry
```

**Public fields** (also directly settable):

| Field | Type | Description |
|---|---|---|
| `origin` | `Vec2` | Bar center in parent-node logical pixels (Y-down) |
| `quad_size` | `f32` | Logical pixel size of the backing square node |
| `pixel_size` | `u32` | Integer up-scale factor (N×N screen px per logical px) |
| `min_geometry` | `BarGeometry` | Geometry at value = 0 |
| `max_geometry` | `BarGeometry` | Geometry at value = 1 |
| `outer_margin` | `f32` | Outline width on the outer radius edge (px) |
| `inner_margin` | `f32` | Outline width on the inner radius edge (px) |
| `angular_margin` | `f32` | Outline width on the angular edges of sectors (px) |
| `fill_color` | `Color` | Color of the lagging fill region |
| `follow_color` | `Color` | Color of the instant-jump lead region |
| `frame_color` | `Color` | Color of the frame outline |
| `background_color` | `Color` | Color behind the bar geometry (transparent by default) |
| `fill_axis` | `FillAxis` | Which axis the value drives |
| `frame_anchor` | `FrameAnchor` | What the frame outline tracks |

**Computing geometry at a value:**

```rust
let geom: BarGeometry = bar.geometry_at(0.5); // geometry for value=0.5
```

### `CircularBarValue`

Drives the animation. Tracks `value` (the target) and `displayed` (the current visible position).

```rust
// Construct
let v = CircularBarValue::new(1.0).with_follow_speed(3.0);

// Update target — fill will lerp to catch up
bar_value.set(0.5);

// Increment target by a delta
bar_value.add(-0.1);
```

**Fields:**

| Field | Type | Description |
|---|---|---|
| `value` | `f32` | Instantaneous target in [0, 1] |
| `displayed` | `f32` | Current displayed value, lerps toward `value` |
| `follow_speed` | `f32` | Lerp rate in value-units per second; `0.0` = instant snap |

### `BarGeometry`

A ring-sector primitive defined in logical pixels relative to the bar's origin.

```rust
let g = BarGeometry::new(inner_radius, outer_radius, start_angle, end_angle);
let ring = BarGeometry::full_ring(inner_radius, outer_radius);

// Collapse for use as min_geometry
let at_zero_angle = g.collapsed_to_start(); // for angular fills
let at_inner = g.collapsed_to_inner();      // for radial fills

// Lerp between two geometries
let mid = g.lerp(other, 0.5);

// Tight bounding box with padding
let (bb_min, bb_max) = g.bounding_box(1.0);
```

### `FillAxis`

Controls which geometric axis animates as the value changes.

| Variant | Behavior |
|---|---|
| `FillAxis::Angle` (default) | End angle moves from `min_geometry.end_angle` to `max_geometry.end_angle` |
| `FillAxis::Radius` | Outer radius moves from `min_geometry.outer_radius` to `max_geometry.outer_radius` |

### `FrameAnchor`

Controls what the frame outline tracks.

| Variant | Behavior |
|---|---|
| `FrameAnchor::Lead` (default) | Frame follows whichever of `value`/`displayed` is ahead — jumps on fill, ghosts on drain |
| `FrameAnchor::Fill` | Frame follows whichever is behind — outline hugs the green region |
| `FrameAnchor::Full` | Frame stays fixed at the bar's full extent regardless of value |

### `ValueBarSystems`

System set labels for ordering your own systems relative to the plugin's pipeline:

```rust
// Run your system between Advance and Sync
app.add_systems(
    Update,
    my_system.after(ValueBarSystems::Advance).before(ValueBarSystems::Sync),
);
```

| Set | What it does |
|---|---|
| `ValueBarSystems::Spawn` | Inserts the `Node` + material for newly-added bars |
| `ValueBarSystems::Advance` | Lerps `displayed` toward `value` each frame |
| `ValueBarSystems::Sync` | Pushes geometry to the shader uniforms |

## Examples

Run the included examples with:

```sh
cargo run --example basic
cargo run --example black_hole
```

**`basic`** — nine bar configurations on one screen: angular fills, radial fills, nested concentric stacks, color banding, slow-follow gauges. Press **1–9** to change the pixel scale; click to modify values.

**`black_hole`** — a single master scalar drives three concentric radial bars with different color bands, demonstrating how to fuse multiple `CircularBar` entities into one coherent HUD element.

## License

Licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
