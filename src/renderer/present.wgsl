@group(0) @binding(0) var scene: texture_2d<f32>;
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs_unorm(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(scene, vec2<i32>(p.xy), 0);
}
@fragment fn fs_srgb(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(scene, vec2<i32>(p.xy), 0);
    let linear = select(pow((c.rgb + 0.055) / 1.055, vec3<f32>(2.4)), c.rgb / 12.92, c.rgb <= vec3<f32>(0.04045));
    return vec4<f32>(linear, c.a);
}
