fn clip_has_bounds(clip_bounds: vec4<f32>) -> bool {
    return clip_bounds.z > 0.0 && clip_bounds.w > 0.0;
}

fn clip_has_rounding(clip_radii: vec4<f32>) -> bool {
    return any(clip_radii > vec4<f32>(0.0));
}

fn clip_corner_radius_for(local_pos: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
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

fn rounded_clip_sdf(local_pos: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let radius = min(
        clip_corner_radius_for(local_pos, size, radii),
        0.5 * min(size.x, size.y),
    );
    let half_size = size * 0.5;
    let centered = local_pos - half_size;
    let q = abs(centered) - (half_size - vec2<f32>(radius));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn clip_slot_alpha(
    clip_bounds: vec4<f32>,
    clip_radii: vec4<f32>,
    point: vec2<f32>,
) -> f32 {
    if !clip_has_bounds(clip_bounds) {
        return 1.0;
    }

    if !clip_has_rounding(clip_radii) {
        let min_bounds = clip_bounds.xy;
        let max_bounds = clip_bounds.xy + clip_bounds.zw;
        let inside = point.x >= min_bounds.x
            && point.x <= max_bounds.x
            && point.y >= min_bounds.y
            && point.y <= max_bounds.y;
        return select(0.0, 1.0, inside);
    }

    let local_pos = point - clip_bounds.xy;
    let sdf = rounded_clip_sdf(local_pos, clip_bounds.zw, clip_radii);
    let aa = max(fwidth(sdf), 1.0);
    return 1.0 - smoothstep(0.0, aa, sdf);
}

fn clip_stack_alpha(
    clip_bounds_0: vec4<f32>,
    clip_radii_0: vec4<f32>,
    clip_bounds_1: vec4<f32>,
    clip_radii_1: vec4<f32>,
    point: vec2<f32>,
) -> f32 {
    return clip_slot_alpha(clip_bounds_0, clip_radii_0, point)
        * clip_slot_alpha(clip_bounds_1, clip_radii_1, point);
}
