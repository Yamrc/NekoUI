struct RectInstance {
    rect: vec4<f32>,
    fill_start_color: vec4<f32>,
    fill_end_color: vec4<f32>,
    fill_meta: vec4<f32>,
    corner_radii: vec4<f32>,
    border_widths: vec4<f32>,
    border_color: vec4<f32>,
    clip_reference: vec4<u32>,
};

@group(1) @binding(0)
var<storage, read> b_rects: array<RectInstance>;

struct ClipSlot {
    clip_bounds: vec4<f32>,
    clip_corner_radii: vec4<f32>,
};

@group(1) @binding(1)
var<storage, read> b_clips: array<ClipSlot>;

struct RectVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) @interpolate(flat) rect_index: u32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> RectVsOut {
    let rect = b_rects[instance_index];
    let point = rect.rect.xy + UNIT_CORNERS[vertex_index] * rect.rect.zw;

    var out: RectVsOut;
    out.position = vec4<f32>(rect_to_ndc(point), 0.0, 1.0);
    out.local_pos = UNIT_CORNERS[vertex_index] * rect.rect.zw;
    out.size = rect.rect.zw;
    out.rect_index = instance_index;
    return out;
}

fn sample_fill(local_pos: vec2<f32>, size: vec2<f32>, rect: RectInstance) -> vec4<f32> {
    if rect.fill_meta.x < 0.5 {
        return rect.fill_start_color;
    }

    let angle = rect.fill_meta.y;
    let dir = vec2<f32>(cos(angle), sin(angle));
    let centered = local_pos - size * 0.5;
    let extent = max(abs(dir.x) * size.x * 0.5 + abs(dir.y) * size.y * 0.5, 0.0001);
    let projection = dot(centered, dir);
    let t = 0.5 + projection / (extent * 2.0);
    return sample_linear_gradient(rect.fill_start_color, rect.fill_end_color, t);
}

fn corner_radius_for(local_pos: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let is_left = local_pos.x < size.x * 0.5;
    let is_top = local_pos.y < size.y * 0.5;
    if is_top {
        if is_left {
            return radii.x;
        }
        return radii.y;
    }
    if is_left {
        return radii.w;
    }
    return radii.z;
}

fn rect_sdf(local_pos: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let radius = min(corner_radius_for(local_pos, size, radii), 0.5 * min(size.x, size.y));
    let half_size = size * 0.5;
    let centered = local_pos - half_size;
    let q = abs(centered) - (half_size - vec2<f32>(radius));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn inner_corner_radii(radii: vec4<f32>, border_widths: vec4<f32>) -> vec4<f32> {
    let top = border_widths.x;
    let right = border_widths.y;
    let bottom = border_widths.z;
    let left = border_widths.w;
    return max(
        radii - vec4<f32>(
            max(left, top),
            max(right, top),
            max(right, bottom),
            max(left, bottom),
        ),
        vec4<f32>(0.0),
    );
}

fn clip_stack_alpha_for(clip_reference: vec4<u32>, point: vec2<f32>) -> f32 {
    var alpha = 1.0;
    var clip_index = 0u;
    loop {
        if clip_index >= clip_reference.y {
            break;
        }
        let clip_slot = b_clips[clip_reference.x + clip_index];
        alpha *= clip_slot_alpha(clip_slot.clip_bounds, clip_slot.clip_corner_radii, point);
        clip_index += 1u;
    }
    return alpha;
}

@fragment
fn fs_main(in: RectVsOut) -> @location(0) vec4<f32> {
    let rect = b_rects[in.rect_index];
    let outer_sdf = rect_sdf(in.local_pos, in.size, rect.corner_radii);
    let aa = max(fwidth(outer_sdf), 1.0);
    let outer_alpha = 1.0 - smoothstep(0.0, aa, outer_sdf);
    let clip_alpha_value = clip_stack_alpha_for(rect.clip_reference, rect.rect.xy + in.local_pos);
    let fill_color = sample_fill(in.local_pos, in.size, rect);

    let has_border = any(rect.border_widths > vec4<f32>(0.0)) && rect.border_color.a > 0.0;
    if !has_border {
        return vec4<f32>(fill_color.rgb, fill_color.a * outer_alpha * clip_alpha_value);
    }

    let inner_origin = vec2<f32>(rect.border_widths.w, rect.border_widths.x);
    let inner_size = vec2<f32>(
        max(in.size.x - (rect.border_widths.w + rect.border_widths.y), 0.0),
        max(in.size.y - (rect.border_widths.x + rect.border_widths.z), 0.0),
    );

    if inner_size.x <= 0.0 || inner_size.y <= 0.0 {
        return vec4<f32>(rect.border_color.rgb, rect.border_color.a * outer_alpha);
    }

    let inner_sdf = rect_sdf(
        in.local_pos - inner_origin,
        inner_size,
        inner_corner_radii(rect.corner_radii, rect.border_widths),
    );
    let inner_alpha = 1.0 - smoothstep(0.0, aa, inner_sdf);
    let color = mix(rect.border_color, fill_color, inner_alpha);
    return vec4<f32>(color.rgb, color.a * outer_alpha * clip_alpha_value);
}
