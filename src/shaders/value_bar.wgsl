// ============================================================================
// CIRCULAR VALUE BAR - Pixel-perfect concentric ring sectors
// ============================================================================
// Each bar describes three nested geometries — frame, lead, fill — over a
// shared origin. They all use the same shape (an inner radius, outer radius,
// start angle, end angle), so the bar renders as a circular sector / ring.
//
// Painting order, from outside in:
//
//   * Frame      — outline of the bar's full extent. Pixels just inside its
//                  edge get painted with `frame_color` to form a margin.
//   * Lead       — the "real" current value. Tracks the instantaneous input.
//   * Fill       — the lagging green value. Lerps toward lead.
//
// Pixels are categorized once per fragment:
//   inside fill                       → fill_color   (green)
//   inside lead   but outside fill    → follow_color (red gap)
//   inside frame  but outside lead    → background   (empty body)
//   inside margin band                → frame_color  (outline)
//   outside frame                     → transparent  (no draw)
//
// All distances are in *logical pixels*. The owner sizes the UI node in
// those same logical pixels; `UiScale` (or any wrapping scale) then renders
// each logical pixel as N screen pixels. Comparisons use crisp, hard
// thresholds so each output pixel resolves to a single color — no blending,
// no half-pixel seams.
// ============================================================================

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct ValueBarUniforms {
    // Mesh size in logical pixels (used to convert UV -> pixel space).
    quad_px_size: vec2<f32>,
    // Center of the bar in mesh-local pixel coordinates.
    center_px: vec2<f32>,

    // Frame (outline) extents.
    frame_outer_radius: f32,
    frame_inner_radius: f32,
    frame_start_angle: f32,
    frame_end_angle: f32,

    // Lead (red boundary) extents — typically tracks the instantaneous value.
    lead_outer_radius: f32,
    lead_inner_radius: f32,
    lead_start_angle: f32,
    lead_end_angle: f32,

    // Fill (green) extents — typically tracks the lerping displayed value.
    fill_outer_radius: f32,
    fill_inner_radius: f32,
    fill_start_angle: f32,
    fill_end_angle: f32,

    // Per-edge frame margin widths in pixels. 0 hides that edge of the
    // outline; the angular value applies to both the start and end edges.
    frame_margin_outer_px: f32,
    frame_margin_inner_px: f32,
    frame_margin_angular_px: f32,
    // Reserved for future use; padded to the 16-byte std140 boundary.
    _pad0: f32,

    fill_color: vec4<f32>,
    follow_color: vec4<f32>,
    frame_color: vec4<f32>,
    background_color: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> bar: ValueBarUniforms;

const TAU: f32 = 6.28318530717958647692;

// Wrap an angle into the half-open interval [start, start + TAU).
fn wrap_angle(angle: f32, start: f32) -> f32 {
    var a = angle - start;
    a = a - floor(a / TAU) * TAU;
    return a;
}

// Test whether a pixel center lies within the angular sector [start, end].
//
// `start` and `end` are absolute angles in radians. The sector sweeps from
// `start` counter-clockwise to `end`. A non-positive sweep is treated as
// empty; a sweep ≥ TAU is treated as full coverage.
fn in_angular_sector(angle: f32, start: f32, end: f32) -> bool {
    let sweep = end - start;
    if sweep <= 0.0 {
        return false;
    }
    if sweep >= TAU {
        return true;
    }
    return wrap_angle(angle, start) <= sweep;
}

// Distance (in radians) from `angle` to the nearest sector edge, measured
// from inside the sector. -1.0 if the sector is empty.
fn angular_distance_to_edge(angle: f32, start: f32, end: f32) -> f32 {
    let sweep = end - start;
    if sweep <= 0.0 {
        return -1.0;
    }
    let from_start = wrap_angle(angle, start);
    return min(from_start, sweep - from_start);
}

fn in_ring_sector(
    r: f32,
    angle: f32,
    inner_radius: f32,
    outer_radius: f32,
    start_angle: f32,
    end_angle: f32,
) -> bool {
    let radial = (r >= inner_radius) && (r < outer_radius);
    if !radial {
        return false;
    }
    return in_angular_sector(angle, start_angle, end_angle);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    // Resolve the pixel center in mesh-local pixel space. Snapping to integer
    // pixels and offsetting by 0.5 places the sample at the geometric center
    // of each output pixel, which keeps concentric circles symmetric.
    let px = floor(in.uv * bar.quad_px_size) + vec2<f32>(0.5, 0.5);
    let offset = px - bar.center_px;
    let r = length(offset);
    let angle = atan2(offset.y, offset.x);

    // ---------------------------------------------------------------
    // Outside the frame: nothing to draw. The background color only
    // fills the unoccupied portion of the frame band — never the
    // surrounding quad — so a solid background reads as a ring, not
    // a filled square.
    // ---------------------------------------------------------------
    let in_frame = in_ring_sector(
        r, angle,
        bar.frame_inner_radius, bar.frame_outer_radius,
        bar.frame_start_angle, bar.frame_end_angle,
    );
    if !in_frame {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // ---------------------------------------------------------------
    // Margin (frame outline): a 1..N pixel band hugging the frame edge.
    // Each edge has its own width so callers can show only the outer
    // outline (color-banded stacks), only the inner outline, etc.
    // The arc-length conversion keeps the angular margin visually uniform
    // around the curve, regardless of the pixel's distance from origin.
    // ---------------------------------------------------------------
    let m_outer = bar.frame_margin_outer_px;
    let m_inner = bar.frame_margin_inner_px;
    let m_angular = bar.frame_margin_angular_px;

    let near_outer = m_outer > 0.0 && r >= (bar.frame_outer_radius - m_outer);
    let near_inner = m_inner > 0.0 && r < (bar.frame_inner_radius + m_inner);

    var near_angular = false;
    if m_angular > 0.0 {
        let frame_sweep = bar.frame_end_angle - bar.frame_start_angle;
        if frame_sweep < TAU && r > 0.0 {
            let arc_margin = m_angular / r;
            let edge_dist = angular_distance_to_edge(
                angle, bar.frame_start_angle, bar.frame_end_angle,
            );
            near_angular = edge_dist >= 0.0 && edge_dist < arc_margin;
        }
    }

    if near_outer || near_inner || near_angular {
        return bar.frame_color;
    }

    // ---------------------------------------------------------------
    // Fill (green) takes precedence over lead (red) takes precedence
    // over the empty frame body.
    // ---------------------------------------------------------------
    let in_fill = in_ring_sector(
        r, angle,
        bar.fill_inner_radius, bar.fill_outer_radius,
        bar.fill_start_angle, bar.fill_end_angle,
    );
    if in_fill {
        return bar.fill_color;
    }

    let in_lead = in_ring_sector(
        r, angle,
        bar.lead_inner_radius, bar.lead_outer_radius,
        bar.lead_start_angle, bar.lead_end_angle,
    );
    if in_lead {
        return bar.follow_color;
    }

    return bar.background_color;
}
