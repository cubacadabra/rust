struct Globals {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    sun_direction: vec4<f32>,
    fog_color: vec4<f32>,
    color_grade: vec4<f32>,
    atmosphere: vec4<f32>,
    lighting: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var world_texture: texture_2d<f32>;

@group(1) @binding(1)
var world_sampler: sampler;

@group(2) @binding(0)
var terrain_textures: texture_2d_array<f32>;

@group(2) @binding(1)
var terrain_sampler: sampler;

struct ShadowGlobals {
    view_projection: mat4x4<f32>,
    texel_size: vec4<f32>,
};

@group(3) @binding(0)
var<uniform> shadow_globals: ShadowGlobals;
@group(3) @binding(1)
var shadow_map: texture_depth_2d;
@group(3) @binding(2)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) tex_coords: vec2<f32>,
    @location(4) image_invert: f32,
    @location(5) texture_bounds: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) tex_coords: vec2<f32>,
    @location(4) image_invert: f32,
    @location(5) texture_bounds: vec4<f32>,
};

fn terrain_hash(point: vec2<f32>) -> f32 {
    return fract(sin(dot(point, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn terrain_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let smooth_local = local * local * (vec2<f32>(3.0) - 2.0 * local);
    let a = terrain_hash(cell);
    let b = terrain_hash(cell + vec2<f32>(1.0, 0.0));
    let c = terrain_hash(cell + vec2<f32>(0.0, 1.0));
    let d = terrain_hash(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, smooth_local.x), mix(c, d, smooth_local.x), smooth_local.y);
}

fn builtin_terrain_color(material: f32, position: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    var surface = position.xz;
    if abs(normal.x) > 0.5 {
        surface = position.zy;
    } else if abs(normal.z) > 0.5 {
        surface = position.xy;
    }
    let broad = terrain_noise(surface * 0.72);
    let grain = terrain_noise(surface * 5.4);
    let flecks = terrain_hash(floor(surface * 13.0));
    let variation = broad * 0.58 + grain * 0.27 + flecks * 0.15;
    var dark = vec3<f32>(0.20, 0.105, 0.032);
    var light = vec3<f32>(0.39, 0.235, 0.082);
    if material > 6.5 {
        dark = vec3<f32>(0.025, 0.16, 0.012);
        light = vec3<f32>(0.11, 0.38, 0.028);
    } else if material < 1.5 {
        if normal.y > 0.72 {
            dark = vec3<f32>(0.025, 0.16, 0.012);
            light = vec3<f32>(0.11, 0.38, 0.028);
        } else if normal.y < -0.72 {
            dark = vec3<f32>(0.20, 0.105, 0.032);
            light = vec3<f32>(0.39, 0.235, 0.082);
        } else {
            dark = vec3<f32>(0.15, 0.105, 0.045);
            light = vec3<f32>(0.31, 0.225, 0.095);
        }
    } else if material < 2.5 {
        dark = vec3<f32>(0.20, 0.105, 0.032);
        light = vec3<f32>(0.39, 0.235, 0.082);
    } else if material < 3.5 {
        dark = vec3<f32>(0.16, 0.17, 0.12);
        light = vec3<f32>(0.36, 0.35, 0.25);
    } else if material < 4.5 {
        dark = vec3<f32>(0.37, 0.25, 0.09);
        light = vec3<f32>(0.68, 0.51, 0.21);
    } else if material < 5.5 {
        dark = vec3<f32>(0.13, 0.065, 0.035);
        light = vec3<f32>(0.29, 0.15, 0.075);
    } else {
        dark = vec3<f32>(0.58, 0.64, 0.66);
        light = vec3<f32>(0.91, 0.94, 0.92);
    }
    return mix(dark, light, 0.18 + variation * 0.82);
}

fn sample_terrain_layer(layer: i32, position: vec3<f32>, normal: vec3<f32>, lod: f32) -> vec3<f32> {
    let weights = pow(abs(normal), vec3<f32>(4.0));
    let normalized_weights = weights / max(weights.x + weights.y + weights.z, 0.0001);
    let scale = 0.18;
    let along_x = textureSampleLevel(terrain_textures, terrain_sampler, position.zy * scale, layer, lod).rgb;
    let along_y = textureSampleLevel(terrain_textures, terrain_sampler, position.xz * scale, layer, lod).rgb;
    let along_z = textureSampleLevel(terrain_textures, terrain_sampler, position.xy * scale, layer, lod).rgb;
    return along_x * normalized_weights.x + along_y * normalized_weights.y + along_z * normalized_weights.z;
}

fn built_in_terrain_texture(material: f32, position: vec3<f32>, normal: vec3<f32>, lod: f32) -> vec3<f32> {
    if material > 6.5 {
        return sample_terrain_layer(0, position, normal, lod);
    }
    if material < 1.5 {
        if normal.y < -0.72 {
            return sample_terrain_layer(2, position, normal, lod);
        }
        let side = sample_terrain_layer(1, position, normal, lod);
        let top = sample_terrain_layer(0, position, normal, lod);
        return mix(side, top, smoothstep(0.35, 0.82, normal.y));
    }
    return sample_terrain_layer(i32(material), position, normal, lod);
}

fn shadow_factor(world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let clip = shadow_globals.view_projection * vec4<f32>(world_position, 1.0);
    if clip.w <= 0.0001 {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    if ndc.z <= 0.0 || ndc.z >= 1.0 || abs(ndc.x) >= 1.0 || abs(ndc.y) >= 1.0 {
        return 1.0;
    }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let light_direction = normalize(-globals.sun_direction.xyz);
    let bias = 0.0015 + (1.0 - max(dot(normal, light_direction), 0.0)) * 0.003;
    let depth = ndc.z - bias;
    var visibility = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y)) * shadow_globals.texel_size.xy
                * shadow_globals.texel_size.z;
            visibility += textureSampleCompare(shadow_map, shadow_sampler, uv + offset, depth);
        }
    }
    return visibility / 9.0;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_projection * vec4<f32>(input.position, 1.0);
    output.world_position = input.position;
    output.normal = input.normal;
    output.color = input.color;
    output.tex_coords = input.tex_coords;
    output.image_invert = input.image_invert;
    output.texture_bounds = input.texture_bounds;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let view_direction = normalize(globals.camera_position.xyz - input.world_position);
    let light_direction = normalize(-globals.sun_direction.xyz);
    let direct_light = max(dot(normal, light_direction), 0.0);
    let shadow = shadow_factor(input.world_position, normal);
    let lighting = dot(globals.lighting.xyz, vec3<f32>(0.33333334))
        + direct_light * 0.21 * globals.lighting.w * shadow;
    let rim = pow(1.0 - max(dot(normal, view_direction), 0.0), 3.0) * 0.06;
    let lit_color = input.color.rgb * lighting + vec3<f32>(rim);
    let distance_to_camera = distance(input.world_position, globals.camera_position.xyz);
        let fog = smoothstep(globals.atmosphere.x, globals.atmosphere.y, distance_to_camera);
    if input.image_invert > 1.5 {
        var terrain = builtin_terrain_color(input.tex_coords.x, input.world_position, normal);
        if input.tex_coords.y > 0.5 {
            let terrain_lod = clamp(log2(max(distance_to_camera * 0.04, 1.0)), 0.0, 9.0);
            terrain = built_in_terrain_texture(input.tex_coords.x, input.world_position, normal, terrain_lod);
        }
        var graded = terrain * input.color.rgb * lighting + vec3<f32>(rim);
        graded *= globals.color_grade.x;
        let luminance = dot(graded, vec3<f32>(0.2126, 0.7152, 0.0722));
        graded = mix(vec3<f32>(luminance), graded, globals.color_grade.z);
        graded = (graded - vec3<f32>(0.5)) * globals.color_grade.y + vec3<f32>(0.5);
        return vec4<f32>(mix(graded, globals.fog_color.rgb, fog), 1.0);
    }
    if input.image_invert > 0.5 {
        let uv = input.texture_bounds.xy + fract(input.tex_coords) * input.texture_bounds.zw;
        let image = textureSampleLevel(world_texture, world_sampler, uv, 0.0);
        if image.a < 0.05 {
            discard;
        }
        var graded = image.rgb * lighting + vec3<f32>(rim);
        graded *= globals.color_grade.x;
        let luminance = dot(graded, vec3<f32>(0.2126, 0.7152, 0.0722));
        graded = mix(vec3<f32>(luminance), graded, globals.color_grade.z);
        graded = (graded - vec3<f32>(0.5)) * globals.color_grade.y + vec3<f32>(0.5);
        return vec4<f32>(mix(graded, globals.fog_color.rgb, fog), image.a);
    }
    var graded = lit_color;
    graded *= globals.color_grade.x;
    let luminance = dot(graded, vec3<f32>(0.2126, 0.7152, 0.0722));
    graded = mix(vec3<f32>(luminance), graded, globals.color_grade.z);
    graded = (graded - vec3<f32>(0.5)) * globals.color_grade.y + vec3<f32>(0.5);
    return vec4<f32>(mix(graded, globals.fog_color.rgb, fog), input.color.a);
}

struct UiVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) image_invert: f32,
};

@group(0) @binding(0)
var ui_texture: texture_2d<f32>;

@group(0) @binding(1)
var ui_sampler: sampler;

@vertex
fn vs_ui(input: VertexInput) -> UiVertexOutput {
    var output: UiVertexOutput;
    output.position = vec4<f32>(input.position.xy, 0.0, 1.0);
    output.color = input.color;
    output.tex_coords = input.tex_coords;
    output.image_invert = input.image_invert;
    return output;
}

@fragment
fn fs_ui(input: UiVertexOutput) -> @location(0) vec4<f32> {
    if input.tex_coords.x < 0.0 {
        return input.color;
    }
    // The atlas has one mip. Explicit LOD is equivalent here and remains
    // valid after the non-uniform solid-color branch on browser WebGPU.
    var image = textureSampleLevel(ui_texture, ui_sampler, input.tex_coords, 0.0);
    if input.image_invert > 0.5 {
        image = vec4<f32>(vec3<f32>(1.0) - image.rgb, image.a);
    }
    return input.color * image;
}
