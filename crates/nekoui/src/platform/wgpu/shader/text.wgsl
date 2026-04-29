@group(1) @binding(0)
var atlas_sampler: sampler;

@group(1) @binding(1)
var atlas_texture: texture_2d<f32>;

struct TextInstance {
    rect: vec4<f32>,
    uv_rect: vec4<f32>,
    payload: vec4<f32>,
    clip_reference: vec4<u32>,
};

@group(2) @binding(0)
var<storage, read> b_text: array<TextInstance>;

struct ClipSlot {
    clip_bounds: vec4<f32>,
    clip_corner_radii: vec4<f32>,
};

@group(2) @binding(1)
var<storage, read> b_clips: array<ClipSlot>;

struct MonoVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) point: vec2<f32>,
    @location(3) @interpolate(flat) text_index: u32,
};

struct ColorVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) point: vec2<f32>,
    @location(3) @interpolate(flat) text_index: u32,
};

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

@vertex
fn vs_mono(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> MonoVsOut {
    let instance = b_text[instance_index];
    let point = instance.rect.xy + UNIT_CORNERS[vertex_index] * instance.rect.zw;

    var out: MonoVsOut;
    out.position = vec4<f32>(rect_to_ndc(point), 0.0, 1.0);
    out.uv = instance.uv_rect.xy + UNIT_CORNERS[vertex_index] * instance.uv_rect.zw;
    out.color = instance.payload;
    out.point = point;
    out.text_index = instance_index;
    return out;
}

@fragment
fn fs_mono(in: MonoVsOut) -> @location(0) vec4<f32> {
    let instance = b_text[in.text_index];
    let sampled_alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;
    let clip_alpha_value = clip_stack_alpha_for(instance.clip_reference, in.point);
    return vec4<f32>(in.color.rgb, sampled_alpha * in.color.a * clip_alpha_value);
}

@vertex
fn vs_color(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> ColorVsOut {
    let instance = b_text[instance_index];
    let point = instance.rect.xy + UNIT_CORNERS[vertex_index] * instance.rect.zw;

    var out: ColorVsOut;
    out.position = vec4<f32>(rect_to_ndc(point), 0.0, 1.0);
    out.uv = instance.uv_rect.xy + UNIT_CORNERS[vertex_index] * instance.uv_rect.zw;
    out.alpha = instance.payload.x;
    out.point = point;
    out.text_index = instance_index;
    return out;
}

@fragment
fn fs_color(in: ColorVsOut) -> @location(0) vec4<f32> {
    let instance = b_text[in.text_index];
    let sampled = textureSample(atlas_texture, atlas_sampler, in.uv);
    let clip_alpha_value = clip_stack_alpha_for(instance.clip_reference, in.point);
    return vec4<f32>(sampled.rgb, sampled.a * in.alpha * clip_alpha_value);
}
