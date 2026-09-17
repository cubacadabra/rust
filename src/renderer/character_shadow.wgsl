struct ShadowGlobals {
    view_projection: mat4x4<f32>,
    texel_size: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> shadow_globals: ShadowGlobals;

struct Input {
    @location(0) position: vec3<f32>,
    @location(3) row0: vec4<f32>,
    @location(4) row1: vec4<f32>,
    @location(5) row2: vec4<f32>,
};

@vertex
fn vs_main(input: Input) -> @builtin(position) vec4<f32> {
    let local = vec4<f32>(input.position, 1.0);
    let world = vec3<f32>(
        dot(input.row0, local),
        dot(input.row1, local),
        dot(input.row2, local),
    );
    return shadow_globals.view_projection * vec4<f32>(world, 1.0);
}
