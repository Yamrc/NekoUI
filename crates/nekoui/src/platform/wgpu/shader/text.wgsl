struct ViewUniForm {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> view: ViewUniForm;

@group(1) @binding(0)
var atlas_sampler: sampler;

@group(1) @binding(1)
var atlas_texture: texture_2d<f32>;

struct TextInstance {
    @location(0) rect: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: TextInstance) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );

    let point = instance.rect.xy + corners[vertex_index] * instance.rect.zw;
    let ndc = vec2<f32>(
        (point.x / view.viewport.x) * 2.0 - 1.0,
        1.0 - (point.y / view.viewport.y) * 2.0,
    );

    var out: VsOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = instance.uv_rect.xy + corners[vertex_index] * instance.uv_rect.zw;
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(atlas_texture, atlas_sampler, in.uv) * in.color;
}
