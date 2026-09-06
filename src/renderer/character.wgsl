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
    @location(3) material: vec4<f32>, @location(4) uv: vec2<f32>,
    @location(5) local_normal_z: f32,
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
    output.uv = input.uv;
    output.local_normal_z = input.normal.z;
    return output;
}
@fragment fn fs_main(input: Output) -> @location(0) vec4<f32> {
    // Derivatives must be evaluated before material branches/discards for
    // WebGPU's uniform-control-flow contract (also valid on Metal and GL).
    let uv_footprint = max(fwidth(input.uv.x), fwidth(input.uv.y));
    // Ordered coverage fade preserves depth and avoids sorting the many rigid
    // pieces as the local camera enters the head. No extra transparency pass.
    if input.tint.a < 1.0 && input.material.z == 0.0 {
        let cell = vec2<u32>(input.position.xy) % vec2<u32>(4u);
        let bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
        if input.tint.a <= (bayer[cell.y * 4u + cell.x] + 0.5) / 16.0 { discard; }
    }
    // Analytic graphic faces keep curved eyes and smiles crisp at close range
    // without extra meshes or a face texture. Discard preserves occlusion.
    if input.material.w >= 4.0 {
        if input.local_normal_z > -0.9 { discard; }
        let p = input.uv * 2.0 - 1.0;
        var color = input.tint.rgb;
        if input.material.w == 4.0 {
            if dot(p, p) > 0.88 { discard; }
            let highlight = (p - vec2<f32>(-0.28, 0.34)) / vec2<f32>(0.23, 0.20);
            if dot(highlight, highlight) < 1.0 { color = vec3<f32>(0.98, 0.97, 0.92); }
        } else if input.material.w == 5.0 {
            let curve = input.material.x;
            let opening = clamp(input.material.y, 0.0, 1.0);
            let smile = curve * (p.x * p.x - 0.45) * 0.75;
            let line = abs(p.y - smile) < 0.13 && abs(p.x) < 0.88;
            let oval = p.x * p.x / 0.64 + p.y * p.y / max(0.02, opening * opening) < 1.0;
            if opening > 0.12 && oval {
                color = vec3<f32>(0.08, 0.05, 0.07);
                // A tiny tooth or tongue cue gives the open expressions a
                // stronger read without adding another mesh or texture.
                if opening > 0.38 && p.y > 0.20 && p.y < 0.62 {
                    color = vec3<f32>(0.96, 0.92, 0.82);
                } else if opening > 0.30 && p.y < -0.36 {
                    color = vec3<f32>(0.82, 0.29, 0.34);
                }
            } else if !line {
                discard;
            }
        } else if input.material.w == 6.0 {
            if dot(p, p) > 0.92 { discard; }
        } else {
            if dot(p, p) > 0.94 { discard; }
            color *= 0.92;
        }
        let fog = smoothstep(100.0, 220.0, distance(input.world, globals.camera_position.xyz));
        return vec4<f32>(mix(color, globals.fog_color.rgb, fog), 1.0);
    }
    let normal = normalize(input.normal);
    let light = normalize(-globals.sun_direction.xyz);
    let view = normalize(globals.camera_position.xyz - input.world);
    let half_vector = normalize(light + view);
    let roughness = clamp(input.material.x, 0.08, 1.0);
    let hemisphere = mix(0.42, 0.72, normal.y * 0.5 + 0.5);
    let diffuse = hemisphere + max(dot(normal, light), 0.0) * 0.46;
    let specular = pow(max(dot(normal, half_vector), 0.0), mix(100.0, 4.0, roughness)) * input.material.y;
    let rim = pow(1.0 - max(dot(normal, view), 0.0), 3.0) * 0.025;
    let base = decode_srgb(input.tint.rgb);
    var lit = base * diffuse + vec3<f32>(specular + rim);
    if input.material.w == 1.0 {
        // A low-frequency weave cue rewards close inspection without adding a
        // texture binding or high-frequency sparkle to distant characters.
        let frequency = 90.0;
        let footprint = uv_footprint * frequency;
        let detail_weight = 1.0 - smoothstep(0.35, 1.5, footprint);
        let weave = sin(input.uv.x * frequency) * sin(input.uv.y * frequency) * 0.028 * detail_weight;
        lit *= 1.0 + weave;
    }
    if input.material.w == 2.0 {
        // Rainwear gets a broad, restrained highlight instead of a fake
        // metallic reflection from a shinier base color.
        lit += vec3<f32>(0.018) * pow(max(dot(normal, view), 0.0), 6.0);
    }
    if input.material.w == 3.0 {
        // Stylized environment response for the soft-metal tier. This is
        // intentionally bounded and remains valid on the ordinary target;
        // reflection probes remain a later quality tier.
        lit += vec3<f32>(0.025, 0.030, 0.038) * (0.5 + 0.5 * normal.y);
    }
    if input.material.z > 0.0 { lit = base * input.material.z; }
    let fog = smoothstep(100.0, 220.0, distance(input.world, globals.camera_position.xyz));
    // Compatibility target stores display-encoded RGB. World/UI keep their
    // historical shading and blending; only character lighting is linear.
    let encoded = encode_srgb(lit);
    let fog_color = select(globals.fog_color.rgb, vec3<f32>(0.0), input.material.z > 0.0);
    return vec4<f32>(mix(encoded, fog_color, fog), input.tint.a);
}
