struct SkyGlobals {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> sky: SkyGlobals;

@vertex
fn vs_sky(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[vertex_index], 0.999, 1.0);
}

@fragment
fn fs_sky(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let height = clamp(
        1.0 - (position.y - sky.viewport.y) / max(sky.viewport.w, 1.0),
        0.0,
        1.0,
    );
    let blend = smoothstep(0.0, 1.0, height);
    return vec4<f32>(mix(sky.horizon.rgb, sky.zenith.rgb, blend), 1.0);
}
