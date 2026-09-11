struct Globals {
    view_projection: mat4x4<f32>, camera_position: vec4<f32>,
    sun_direction: vec4<f32>, fog_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var morph_texture: texture_2d<f32>;
@group(1) @binding(1) var morph_sampler: sampler;

struct Input {
    @location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>,
    @location(3) row0: vec4<f32>, @location(4) row1: vec4<f32>, @location(5) row2: vec4<f32>,
    @location(6) normal0: vec4<f32>, @location(7) normal1: vec4<f32>, @location(8) normal2: vec4<f32>,
    @location(9) tint: vec4<f32>, @location(10) material: vec4<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>, @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>, @location(2) tint: vec4<f32>,
    @location(3) material: vec4<f32>, @location(4) uv: vec2<f32>,
};
fn decode_srgb(v: vec3<f32>) -> vec3<f32> {
    return select(pow((v + 0.055) / 1.055, vec3<f32>(2.4)), v / 12.92, v <= vec3<f32>(0.04045));
}
fn encode_srgb(v: vec3<f32>) -> vec3<f32> {
    let c = max(v, vec3<f32>(0.0));
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}
@vertex fn vs_main(input: Input) -> Output {
    var output: Output;
    let position = vec4<f32>(input.position, 1.0);
    output.world = vec3<f32>(dot(input.row0, position), dot(input.row1, position), dot(input.row2, position));
    output.position = globals.view_projection * vec4<f32>(output.world, 1.0);
    output.normal = vec3<f32>(dot(input.normal0.xyz, input.normal), dot(input.normal1.xyz, input.normal), dot(input.normal2.xyz, input.normal));
    output.tint = input.tint;
    output.material = input.material;
    output.uv = input.uv;
    return output;
}
@fragment fn fs_main(input: Output) -> @location(0) vec4<f32> {
    let sampled = textureSample(morph_texture, morph_sampler, input.uv);
    if sampled.a < 0.08 { discard; }
    let normal = normalize(input.normal);
    let light = normalize(-globals.sun_direction.xyz);
    let view = normalize(globals.camera_position.xyz - input.world);
    let half_vector = normalize(light + view);
    let roughness = clamp(input.material.x, 0.08, 1.0);
    let hemisphere = mix(0.42, 0.72, normal.y * 0.5 + 0.5);
    let diffuse = hemisphere + max(dot(normal, light), 0.0) * 0.46;
    let specular = pow(max(dot(normal, half_vector), 0.0), mix(100.0, 4.0, roughness)) * input.material.y;
    let rim = pow(1.0 - max(dot(normal, view), 0.0), 3.0) * 0.025;
    let base = decode_srgb(input.tint.rgb * sampled.rgb);
    let lit = base * diffuse + vec3<f32>(specular + rim);
    let fog = smoothstep(100.0, 220.0, distance(input.world, globals.camera_position.xyz));
    return vec4<f32>(mix(encode_srgb(lit), globals.fog_color.rgb, fog), input.tint.a * sampled.a);
}
