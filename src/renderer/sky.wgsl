struct SkyGlobals {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    viewport: vec4<f32>,
    // Pixel-space center, elevation, visibility.
    sun: vec4<f32>,
    // Elapsed time, cloud contrast, cloud opacity, reserved.
    clouds: vec4<f32>,
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

fn hash(point: vec2<f32>) -> f32 {
    return fract(sin(dot(point, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let eased = local * local * (vec2<f32>(3.0) - 2.0 * local);
    return mix(
        mix(hash(cell), hash(cell + vec2<f32>(1.0, 0.0)), eased.x),
        mix(hash(cell + vec2<f32>(0.0, 1.0)), hash(cell + vec2<f32>(1.0, 1.0)), eased.x),
        eased.y,
    );
}

fn soft_clouds(uv: vec2<f32>) -> f32 {
    let drift = vec2<f32>(sky.clouds.x * 0.004, sky.clouds.x * 0.0015);
    let low = noise(uv * vec2<f32>(2.6, 4.2) + drift);
    let detail = noise(uv * vec2<f32>(7.0, 10.0) + drift * 1.7);
    let broken = smoothstep(0.32, 0.58, low * 0.72 + detail * 0.28);
    // A second, broad warped band layer prevents the value-noise layer from
    // reading as tiny isolated puffs at gameplay distance. It gives the sky
    // the long soft wisps expected from a bright morning, without a texture.
    let wisp = 0.5 + 0.5 * sin(uv.y * 23.0 + sin(uv.x * 3.8) * 2.4 + uv.x * 4.5);
    let sheet = 0.5 + 0.5 * sin(uv.x * 5.1 - uv.y * 2.7);
    let wisps = smoothstep(0.58, 0.92, wisp) * smoothstep(0.30, 0.72, sheet);
    return max(broken * 0.80, wisps * 0.75);
}

@fragment
fn fs_sky(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let height = clamp(
        1.0 - (position.y - sky.viewport.y) / max(sky.viewport.w, 1.0),
        0.0,
        1.0,
    );
    let blend = smoothstep(0.0, 1.0, pow(height, 0.78));
    var color = mix(sky.horizon.rgb, sky.zenith.rgb, blend);

    // Pale mist gives the low sky a soft morning horizon rather than a hard
    // two-color seam. It is intentionally subtle at higher elevations.
    let haze = exp(-height * 7.0) * 0.32;
    color = mix(color, vec3<f32>(0.88, 0.90, 0.79), haze);

    let sun_offset = (position.xy - sky.sun.xy)
        / max(vec2<f32>(sky.viewport.z, sky.viewport.w), vec2<f32>(1.0));
    let sun_distance = length(sun_offset);
    let visible = sky.sun.w * smoothstep(-0.08, 0.08, sky.sun.z);
    let warm = vec3<f32>(1.0, 0.64, 0.30);
    let halo = exp(-sun_distance * sun_distance * 170.0) * visible;
    let disk = smoothstep(0.012, 0.004, sun_distance) * visible;
    color += warm * (halo * 0.32 + disk * 0.96);

    // Broken screen-space cloud cells are a cheap atmospheric layer, not an
    // attempt to reproduce proprietary sky textures. They fade at the bright
    // horizon and do not obscure the low morning sun disk.
    let normalized = (position.xy - sky.viewport.xy)
        / max(sky.viewport.zw, vec2<f32>(1.0));
    let cloud_height = smoothstep(0.03, 0.16, height);
    let cloud = soft_clouds(normalized) * cloud_height * sky.clouds.z;
    let silver = vec3<f32>(0.80, 0.89, 0.93);
    color = mix(color, silver, cloud * (0.52 + halo * 0.14));
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
