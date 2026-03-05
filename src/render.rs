use std::cell::Cell;

use crate::state::*;

const BG_COLOR: (u8, u8, u8) = (30, 30, 30);

// Thread-local clip rectangle for tiled rendering.
// When set, set_pixel will reject pixels outside these bounds.
thread_local! {
    static CLIP_RECT: Cell<Option<(i32, i32, i32, i32)>> = const { Cell::new(None) }; // (x0, y0, x1, y1) inclusive
}

fn set_clip(x0: i32, y0: i32, x1: i32, y1: i32) {
    CLIP_RECT.with(|c| c.set(Some((x0, y0, x1, y1))));
}

fn clear_clip() {
    CLIP_RECT.with(|c| c.set(None));
}

/// Render the full graph into an RGBA pixel buffer.
pub fn render_graph(state: &GraphState) -> Vec<u8> {
    let w = state.width;
    let h = state.height;
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let mut buf = vec![0u8; (w * h * 4) as usize];

    // Fill background
    for i in 0..(w * h) as usize {
        buf[i * 4] = BG_COLOR.0;
        buf[i * 4 + 1] = BG_COLOR.1;
        buf[i * 4 + 2] = BG_COLOR.2;
        buf[i * 4 + 3] = 255;
    }

    // Draw edges
    for edge in &state.edges {
        let (fx, fy) = commit_position(edge.from_lane, edge.from_row);
        let (tx, ty) = commit_position(edge.to_lane, edge.to_row);
        let (sfx, sfy) = world_to_screen(fx, fy, state);
        let (stx, sty) = world_to_screen(tx, ty, state);

        let color = LANE_COLORS[edge.color_index % LANE_COLORS.len()];

        if edge.from_lane == edge.to_lane {
            draw_line(&mut buf, w, h, sfx, sfy, stx, sty, color, 2.0);
        } else {
            let mid_y = (sfy + sty) / 2.0;
            draw_bezier(&mut buf, w, h, sfx, sfy, sfx, mid_y, stx, mid_y, stx, sty, color, 2.0);
        }
    }

    // Draw nodes + commit info
    for commit in &state.commits {
        let (wx, wy) = commit_position(commit.lane, commit.row);
        let (sx, sy) = world_to_screen(wx, wy, state);
        let r = NODE_RADIUS * state.zoom;
        let color = LANE_COLORS[commit.lane % LANE_COLORS.len()];

        draw_filled_circle(&mut buf, w, h, sx, sy, r, color);

        // HEAD indicator: gold ring
        if commit.is_head {
            draw_circle_outline(&mut buf, w, h, sx, sy, r + 3.0 * state.zoom, (255, 215, 0), 2.0);
        }

        // Selected indicator: white ring
        let is_selected = state.selected_oid.as_deref() == Some(&commit.oid);
        if is_selected {
            draw_circle_outline(&mut buf, w, h, sx, sy, r + 2.0 * state.zoom, (255, 255, 255), 2.0);
        }

        // Commit info text: short OID + message, rendered to the right of the rightmost lane
        let text_x = {
            let rightmost = (state.max_lanes.max(1)) as f32 * LANE_WIDTH;
            let (rx, _) = world_to_screen(rightmost, wy, state);
            rx + 12.0
        };

        // Branch labels on the node line (colored tag)
        let mut label_x = text_x;
        if !commit.branch_labels.is_empty() {
            let label = commit.branch_labels.join(", ");
            draw_text_simple(&mut buf, w, h, label_x, sy, &label, (255, 215, 0));
            label_x += (label.len() as f32) * 6.0 + 8.0;
        }

        // Short OID
        let oid_color = if is_selected { (255, 255, 255) } else { (97, 175, 239) };
        draw_text_simple(&mut buf, w, h, label_x, sy, &commit.short_oid, oid_color);
        label_x += (commit.short_oid.len() as f32) * 6.0 + 8.0;

        // Commit message (truncated)
        let msg: String = commit.message.chars().take(50).collect();
        let msg_color = if is_selected { (220, 220, 220) } else { (171, 178, 191) };
        draw_text_simple(&mut buf, w, h, label_x, sy, &msg, msg_color);
    }

    buf
}

/// Set a pixel with alpha blending, respecting the thread-local clip rect.
#[inline(always)]
fn set_pixel(buf: &mut [u8], w: u32, h: u32, x: i32, y: i32, r: u8, g: u8, b: u8, a: f32) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return;
    }
    // Check clip rect
    if let Some((cx0, cy0, cx1, cy1)) = CLIP_RECT.with(|c| c.get()) {
        if x < cx0 || x > cx1 || y < cy0 || y > cy1 {
            return;
        }
    }
    let idx = (y as u32 * w + x as u32) as usize * 4;
    if idx + 3 >= buf.len() {
        return;
    }
    let alpha = a.clamp(0.0, 1.0);
    let inv = 1.0 - alpha;
    buf[idx] = (r as f32 * alpha + buf[idx] as f32 * inv) as u8;
    buf[idx + 1] = (g as f32 * alpha + buf[idx + 1] as f32 * inv) as u8;
    buf[idx + 2] = (b as f32 * alpha + buf[idx + 2] as f32 * inv) as u8;
    buf[idx + 3] = 255;
}

/// Draw a filled circle with anti-aliased edges.
fn draw_filled_circle(buf: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, color: (u8, u8, u8)) {
    let x0 = (cx - r - 1.0).floor() as i32;
    let x1 = (cx + r + 1.0).ceil() as i32;
    let y0 = (cy - r - 1.0).floor() as i32;
    let y1 = (cy + r + 1.0).ceil() as i32;

    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = px as f32 - cx;
            let dy = py as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= r - 0.5 {
                set_pixel(buf, w, h, px, py, color.0, color.1, color.2, 1.0);
            } else if dist <= r + 0.5 {
                let alpha = 1.0 - (dist - (r - 0.5));
                set_pixel(buf, w, h, px, py, color.0, color.1, color.2, alpha);
            }
        }
    }
}

/// Draw a circle outline.
fn draw_circle_outline(buf: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, r: f32, color: (u8, u8, u8), thickness: f32) {
    let x0 = (cx - r - thickness).floor() as i32;
    let x1 = (cx + r + thickness).ceil() as i32;
    let y0 = (cy - r - thickness).floor() as i32;
    let y1 = (cy + r + thickness).ceil() as i32;

    let half = thickness / 2.0;

    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = px as f32 - cx;
            let dy = py as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let ring_dist = (dist - r).abs();
            if ring_dist <= half + 0.5 {
                let alpha = (1.0 - (ring_dist - half).max(0.0)).clamp(0.0, 1.0);
                set_pixel(buf, w, h, px, py, color.0, color.1, color.2, alpha);
            }
        }
    }
}

/// Draw an anti-aliased line using distance-to-line-segment approach.
/// Instead of stepping along the line and scanning area per step,
/// we iterate pixels in the bounding box and compute distance to the segment.
fn draw_line(buf: &mut [u8], w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32, color: (u8, u8, u8), thickness: f32) {
    let half = thickness / 2.0 + 0.5;

    let min_x = x0.min(x1) - half;
    let max_x = x0.max(x1) + half;
    let min_y = y0.min(y1) - half;
    let max_y = y0.max(y1) + half;

    // Clamp to screen bounds
    let px0 = (min_x.floor() as i32).max(0);
    let px1 = (max_x.ceil() as i32).min(w as i32 - 1);
    let py0 = (min_y.floor() as i32).max(0);
    let py1 = (max_y.ceil() as i32).min(h as i32 - 1);

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 0.001 {
        // Degenerate: just a point
        set_pixel(buf, w, h, x0 as i32, y0 as i32, color.0, color.1, color.2, 1.0);
        return;
    }

    let inv_len_sq = 1.0 / len_sq;

    for py in py0..=py1 {
        for px in px0..=px1 {
            let pxf = px as f32;
            let pyf = py as f32;

            // Project point onto line segment, clamp t to [0,1]
            let t = ((pxf - x0) * dx + (pyf - y0) * dy) * inv_len_sq;
            let t = t.clamp(0.0, 1.0);

            let closest_x = x0 + t * dx;
            let closest_y = y0 + t * dy;

            let ddx = pxf - closest_x;
            let ddy = pyf - closest_y;
            let dist = (ddx * ddx + ddy * ddy).sqrt();

            if dist <= half {
                let alpha = (1.0 - (dist - (half - 1.0)).max(0.0)).clamp(0.0, 1.0);
                set_pixel(buf, w, h, px, py, color.0, color.1, color.2, alpha);
            }
        }
    }
}

/// Draw a cubic bezier curve.
fn draw_bezier(
    buf: &mut [u8], w: u32, h: u32,
    x0: f32, y0: f32,
    cx0: f32, cy0: f32,
    cx1: f32, cy1: f32,
    x1: f32, y1: f32,
    color: (u8, u8, u8),
    thickness: f32,
) {
    let steps = 20;
    let mut prev_x = x0;
    let mut prev_y = y0;

    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let px = u * u * u * x0 + 3.0 * u * u * t * cx0 + 3.0 * u * t * t * cx1 + t * t * t * x1;
        let py = u * u * u * y0 + 3.0 * u * u * t * cy0 + 3.0 * u * t * t * cy1 + t * t * t * y1;

        draw_line(buf, w, h, prev_x, prev_y, px, py, color, thickness);
        prev_x = px;
        prev_y = py;
    }
}

/// Draw simple text (5x7 bitmap font, only ASCII printable).
fn draw_text_simple(buf: &mut [u8], w: u32, h: u32, x: f32, y: f32, text: &str, color: (u8, u8, u8)) {
    let mut cx = x as i32;
    let cy = (y - 3.0) as i32;

    for ch in text.chars().take(60) {
        let glyph = get_glyph(ch);
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    set_pixel(buf, w, h, cx + col, cy + row as i32, color.0, color.1, color.2, 1.0);
                }
            }
        }
        cx += 6;
    }
}

/// Minimal 5x7 bitmap font - returns 7 rows of bit patterns.
fn get_glyph(ch: char) -> [u8; 7] {
    match ch {
        'a'..='z' => get_glyph((ch as u8 - b'a' + b'A') as char),
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b01110, 0b00001],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        ' ' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        ':' => [0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '*' => [0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000],
        '#' => [0b01010, 0b11111, 0b01010, 0b01010, 0b11111, 0b01010, 0b00000],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111], // box for unknown
    }
}

/// Calculate grid layout for N repos: returns (cols, rows).
fn grid_layout(n: usize) -> (usize, usize) {
    if n <= 1 {
        (1, 1)
    } else if n <= 2 {
        (2, 1)
    } else if n <= 4 {
        (2, 2)
    } else if n <= 6 {
        (3, 2)
    } else {
        let cols = 3;
        let rows = (n + cols - 1) / cols;
        (cols, rows)
    }
}

/// Render multiple repo graphs tiled into a single pixel buffer.
pub fn render_multi_graph(state: &crate::state::AppState, total_w: u32, total_h: u32) -> Vec<u8> {
    let n = state.repos.len();
    if n == 0 || total_w == 0 || total_h == 0 {
        return Vec::new();
    }

    // Single repo: delegate to existing render
    if n == 1 {
        return render_graph(&state.repos[0].graph);
    }

    let mut buf = vec![0u8; (total_w * total_h * 4) as usize];

    // Fill background
    for i in 0..(total_w * total_h) as usize {
        buf[i * 4] = BG_COLOR.0;
        buf[i * 4 + 1] = BG_COLOR.1;
        buf[i * 4 + 2] = BG_COLOR.2;
        buf[i * 4 + 3] = 255;
    }

    let (cols, rows) = grid_layout(n);
    let cell_w = total_w / cols as u32;
    let cell_h = total_h / rows as u32;

    for (idx, repo) in state.repos.iter().enumerate() {
        let col = idx % cols;
        let row = idx / cols;
        let ox = col as u32 * cell_w;
        let oy = row as u32 * cell_h;

        // Draw cell border
        let border_color: (u8, u8, u8) = (68, 68, 68);
        // Top border
        for x in ox..ox + cell_w {
            set_pixel(&mut buf, total_w, total_h, x as i32, oy as i32, border_color.0, border_color.1, border_color.2, 1.0);
        }
        // Left border
        for y in oy..oy + cell_h {
            set_pixel(&mut buf, total_w, total_h, ox as i32, y as i32, border_color.0, border_color.1, border_color.2, 1.0);
        }

        // Draw repo name header
        let header_h = 18u32;
        let header_color: (u8, u8, u8) = (45, 45, 45);
        for y in oy + 1..oy + header_h {
            for x in ox + 1..ox + cell_w {
                set_pixel(&mut buf, total_w, total_h, x as i32, y as i32, header_color.0, header_color.1, header_color.2, 1.0);
            }
        }
        draw_text_simple(&mut buf, total_w, total_h, (ox + 6) as f32, (oy + 10) as f32, &repo.name, (97, 175, 239));

        // Render graph in the remaining area by creating a temporary adjusted state
        let graph_oy = oy + header_h;
        let graph_h = cell_h.saturating_sub(header_h);
        if graph_h == 0 {
            continue;
        }

        // Clip rendering to this cell's bounds
        set_clip(ox as i32, graph_oy as i32, (ox + cell_w - 1) as i32, (oy + cell_h - 1) as i32);

        // Render each commit/edge with cell offset applied
        let gs = &repo.graph;
        // Draw edges
        for edge in &gs.edges {
            let (fx, fy) = commit_position(edge.from_lane, edge.from_row);
            let (tx, ty) = commit_position(edge.to_lane, edge.to_row);
            let (sfx, sfy) = world_to_screen(fx, fy, gs);
            let (stx, sty) = world_to_screen(tx, ty, gs);
            let sfx = sfx + ox as f32;
            let sfy = sfy + graph_oy as f32;
            let stx = stx + ox as f32;
            let sty = sty + graph_oy as f32;

            let color = LANE_COLORS[edge.color_index % LANE_COLORS.len()];

            if edge.from_lane == edge.to_lane {
                draw_line(&mut buf, total_w, total_h, sfx, sfy, stx, sty, color, 2.0);
            } else {
                let mid_y = (sfy + sty) / 2.0;
                draw_bezier(&mut buf, total_w, total_h, sfx, sfy, sfx, mid_y, stx, mid_y, stx, sty, color, 2.0);
            }
        }

        // Draw nodes + labels
        for commit in &gs.commits {
            let (wx, wy) = commit_position(commit.lane, commit.row);
            let (sx, sy) = world_to_screen(wx, wy, gs);
            let sx = sx + ox as f32;
            let sy = sy + graph_oy as f32;
            let r = NODE_RADIUS * gs.zoom;
            let color = LANE_COLORS[commit.lane % LANE_COLORS.len()];

            draw_filled_circle(&mut buf, total_w, total_h, sx, sy, r, color);

            if commit.is_head {
                draw_circle_outline(&mut buf, total_w, total_h, sx, sy, r + 3.0 * gs.zoom, (255, 215, 0), 2.0);
            }

            let is_selected = gs.selected_oid.as_deref() == Some(&commit.oid);
            if is_selected {
                draw_circle_outline(&mut buf, total_w, total_h, sx, sy, r + 2.0 * gs.zoom, (255, 255, 255), 2.0);
            }

            // Commit info text
            let text_x = {
                let rightmost = (gs.max_lanes.max(1)) as f32 * LANE_WIDTH;
                let (rx, _) = world_to_screen(rightmost, wy, gs);
                rx + ox as f32 + 12.0
            };

            let mut label_x = text_x;
            if !commit.branch_labels.is_empty() {
                let label = commit.branch_labels.join(", ");
                draw_text_simple(&mut buf, total_w, total_h, label_x, sy, &label, (255, 215, 0));
                label_x += (label.len() as f32) * 6.0 + 8.0;
            }

            let oid_color = if is_selected { (255, 255, 255) } else { (97, 175, 239) };
            draw_text_simple(&mut buf, total_w, total_h, label_x, sy, &commit.short_oid, oid_color);
            label_x += (commit.short_oid.len() as f32) * 6.0 + 8.0;

            let msg: String = commit.message.chars().take(30).collect();
            let msg_color = if is_selected { (220, 220, 220) } else { (171, 178, 191) };
            draw_text_simple(&mut buf, total_w, total_h, label_x, sy, &msg, msg_color);
        }

        clear_clip();
    }

    buf
}

/// Hit test for multi-repo view. Returns (repo_index, oid).
pub fn hit_test_multi(state: &crate::state::AppState, sx: f32, sy: f32, total_w: u32, total_h: u32) -> Option<(usize, String)> {
    let n = state.repos.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return hit_test(&state.repos[0].graph, sx, sy).map(|oid| (0, oid));
    }

    let (cols, _rows) = grid_layout(n);
    let cell_w = total_w as f32 / cols as f32;
    let cell_h = total_h as f32 / ((n + cols - 1) / cols) as f32;
    let header_h = 18.0f32;

    let col = (sx / cell_w) as usize;
    let row = ((sy) / cell_h) as usize;
    let idx = row * cols + col;

    if idx >= n {
        return None;
    }

    let ox = col as f32 * cell_w;
    let oy = row as f32 * cell_h + header_h;

    // Transform screen coords to local graph coords
    let local_sx = sx - ox;
    let local_sy = sy - oy;

    let gs = &state.repos[idx].graph;
    let hit_radius = NODE_RADIUS * gs.zoom + 4.0;
    for commit in &gs.commits {
        let (wx, wy) = commit_position(commit.lane, commit.row);
        let (cx, cy) = world_to_screen(wx, wy, gs);
        let dx = local_sx - cx;
        let dy = local_sy - cy;
        if dx * dx + dy * dy <= hit_radius * hit_radius {
            return Some((idx, commit.oid.clone()));
        }
    }
    None
}

/// Determine which repo cell the mouse is in for multi-repo view.
pub fn cell_index_at(sx: f32, sy: f32, n: usize, total_w: u32, total_h: u32) -> Option<usize> {
    if n <= 1 {
        return Some(0);
    }
    let (cols, _rows) = grid_layout(n);
    let cell_w = total_w as f32 / cols as f32;
    let rows = (n + cols - 1) / cols;
    let cell_h = total_h as f32 / rows as f32;

    let col = (sx / cell_w) as usize;
    let row = (sy / cell_h) as usize;
    let idx = row * cols + col;
    if idx < n { Some(idx) } else { None }
}

/// Hit test: check if screen coordinates (sx, sy) are near a commit node.
pub fn hit_test(state: &GraphState, sx: f32, sy: f32) -> Option<String> {
    let hit_radius = NODE_RADIUS * state.zoom + 4.0;
    for commit in &state.commits {
        let (wx, wy) = commit_position(commit.lane, commit.row);
        let (cx, cy) = world_to_screen(wx, wy, state);
        let dx = sx - cx;
        let dy = sy - cy;
        if dx * dx + dy * dy <= hit_radius * hit_radius {
            return Some(commit.oid.clone());
        }
    }
    None
}
