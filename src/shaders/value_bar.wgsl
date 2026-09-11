// ============================================================================
// CIRCULAR VALUE BAR - Pixel-perfect concentric ring sectors
// ============================================================================
// Each bar describes three nested geometries — frame, lead, fill — over a
// shared origin. They all use the same shape (an inner radius, outer radius,
// start angle, end angle), so the bar renders as a circular sector / ring.
//
// Painting order, from outside in:
//
//   * Margin      — pixels outside the frame but within margin distance of
//                   its edges. Painted with `frame_color` to form the outline.
//   * Frame       — the active zone whose edges the outline wraps around.
//   * Lead        — the "real" current value. Tracks the instantaneous input.
//   * Fill        — the lagging green value. Lerps toward lead.
//
// Pixels are categorized once per fragment:
//   outside clip (frame + margins)       → transparent  (no draw)
//   outside frame but inside clip        → frame_color  (outline)
//   inside fill                          → fill_color   (green)
//   inside lead   but outside fill       → follow_color (red gap)
//   inside frame  but outside lead       → background   (empty body)
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

    // Frame (active zone) extents — the outline wraps outside these edges.
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

    let m_outer = bar.frame_margin_outer_px;
    let m_inner = bar.frame_margin_inner_px;
    let m_angular = bar.frame_margin_angular_px;
    let frame_sweep = bar.frame_end_angle - bar.frame_start_angle;

    // ---------------------------------------------------------------
    // Clip test: the visible region is the frame geometry expanded
    // outward by margins. Pixels outside this are transparent.
    // ---------------------------------------------------------------

    // Radial clip — expand outward by per-edge margin widths.
    let clip_inner = bar.frame_inner_radius - m_inner;
    let clip_outer = bar.frame_outer_radius + m_outer;
    if r < clip_inner || r >= clip_outer {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Angular clip — expand each edge by the arc-length-equivalent angle
    // at this pixel's radius so the margin stays a constant pixel width.
    if frame_sweep <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    if frame_sweep < TAU {
        var clip_start = bar.frame_start_angle;
        var clip_end = bar.frame_end_angle;
        if m_angular > 0.0 && r > 0.0 {
            let angular_pad = m_angular / r;
            clip_start -= angular_pad;
            clip_end += angular_pad;
        }
        if !in_angular_sector(angle, clip_start, clip_end) {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }

    // ---------------------------------------------------------------
    // Margin: pixels inside the clip but outside the original frame
    // geometry are the outline band. The per-edge margin widths are
    // already baked into the clip bounds, so a zero-width edge
    // produces no expansion and no outline on that side.
    // ---------------------------------------------------------------
    let in_frame = in_ring_sector(
        r, angle,
        bar.frame_inner_radius, bar.frame_outer_radius,
        bar.frame_start_angle, bar.frame_end_angle,
    );
    if !in_frame {
        return bar.frame_color;
    }

    // ---------------------------------------------------------------
    // Inside the frame: fill (green) > lead (red) > background.
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
