@group(0) @binding(0) var scene: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, 1.0 - y);
    return out;
}

@fragment fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let size = textureDimensions(scene);
    let pixel = vec2<i32>(clamp(in.uv * vec2<f32>(size), vec2<f32>(0.0), vec2<f32>(size - vec2<u32>(1u))));
    return textureLoad(scene, pixel, 0);
}
