struct PostGlobals {
    // Matches Roblox ColorCorrectionEffect's additive Brightness and delta
    // Contrast/Saturation semantics instead of material-local multipliers.
    color_correction: vec4<f32>,
    // Normalized sun center, radial intensity, and ray spread.
    sun_rays: vec4<f32>,
};

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var<uniform> post: PostGlobals;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

fn scene_color(uv: vec2<f32>) -> vec3<f32> {
    let extent = vec2<i32>(textureDimensions(scene));
    let pixel = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.99999)) * vec2<f32>(extent));
    return textureLoad(scene, pixel, 0).rgb;
}

fn grade(color: vec3<f32>) -> vec3<f32> {
    var corrected = max(color + vec3<f32>(post.color_correction.x), vec3<f32>(0.0));
    corrected = (corrected - vec3<f32>(0.5)) * (1.0 + post.color_correction.y) + vec3<f32>(0.5);
    let luminance = dot(corrected, vec3<f32>(0.2126, 0.7152, 0.0722));
    corrected = mix(vec3<f32>(luminance), corrected, 1.0 + post.color_correction.z);
    return clamp(corrected, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sun_rays(uv: vec2<f32>) -> vec3<f32> {
    let intensity = post.sun_rays.z;
    if intensity <= 0.00001 {
        return vec3<f32>(0.0);
    }
    // A short, fixed tap chain keeps the effect portable to WebGL and mobile
    // while still pulling bright sky and sun pixels outward from the source.
    let step_to_sun = (uv - post.sun_rays.xy) * post.sun_rays.w * 0.32 / 7.0;
    var coordinate = uv;
    var scattering = vec3<f32>(0.0);
    var weight = 1.0;
    for (var index = 0; index < 7; index++) {
        coordinate -= step_to_sun;
        let sample = scene_color(coordinate);
        let luminance = dot(sample, vec3<f32>(0.2126, 0.7152, 0.0722));
        scattering += sample * smoothstep(0.34, 1.0, luminance) * weight;
        weight *= 0.78;
    }
    // A low warm tint matches a sunrise shaft without adding Bloom.
    return scattering * vec3<f32>(1.0, 0.88, 0.68) * intensity * 0.23;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let extent = vec2<f32>(textureDimensions(scene));
    let uv = position.xy / max(extent, vec2<f32>(1.0));
    let corrected = grade(scene_color(uv));
    return vec4<f32>(clamp(corrected + sun_rays(uv), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
