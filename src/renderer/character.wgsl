struct Globals {
    view_projection: mat4x4<f32>, camera_position: vec4<f32>,
    sun_direction: vec4<f32>, fog_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> globals: Globals;

struct Input {
    @location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>,
    @location(3) row0: vec4<f32>, @location(4) row1: vec4<f32>, @location(5) row2: vec4<f32>,
    @location(6) normal0: vec4<f32>, @location(7) normal1: vec4<f32>, @location(8) normal2: vec4<f32>,
    @location(9) tint: vec4<f32>, @location(10) material: vec4<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>, @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>, @location(2) tint: vec4<f32>,
    @location(3) material: vec4<f32>,
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
    let p = vec4<f32>(input.position, 1.0);
    output.world = vec3<f32>(dot(input.row0, p), dot(input.row1, p), dot(input.row2, p));
    output.position = globals.view_projection * vec4<f32>(output.world, 1.0);
    output.normal = vec3<f32>(dot(input.normal0.xyz, input.normal), dot(input.normal1.xyz, input.normal), dot(input.normal2.xyz, input.normal));
    output.tint = input.tint;
    output.material = input.material;
    return output;
}
@fragment fn fs_main(input: Output) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let light = normalize(-globals.sun_direction.xyz);
    let view = normalize(globals.camera_position.xyz - input.world);
    let half_vector = normalize(light + view);
    let roughness = clamp(input.material.x, 0.08, 1.0);
    let diffuse = 0.66 + max(dot(normal, light), 0.0) * 0.5;
    let specular = pow(max(dot(normal, half_vector), 0.0), mix(100.0, 4.0, roughness)) * input.material.y;
    let rim = pow(1.0 - max(dot(normal, view), 0.0), 3.0) * 0.025;
    let base = decode_srgb(input.tint.rgb);
    var lit = base * diffuse + vec3<f32>(specular + rim);
    if input.material.z > 0.0 { lit = base * input.material.z; }
    let fog = smoothstep(52.0, 115.0, distance(input.world, globals.camera_position.xyz));
    // Compatibility target stores display-encoded RGB. World/UI keep their
    // historical shading and blending; only character lighting is linear.
    let encoded = encode_srgb(lit);
    let fog_color = select(globals.fog_color.rgb, vec3<f32>(0.0), input.material.z > 0.0);
    return vec4<f32>(mix(encoded, fog_color, fog), input.tint.a);
}
