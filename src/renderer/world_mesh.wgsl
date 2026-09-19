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
@group(3) @binding(0) var<uniform> shadow_globals: ShadowGlobals;
@group(3) @binding(1) var shadow_map: texture_depth_2d;
@group(3) @binding(2) var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) row0: vec4<f32>,
    @location(4) row1: vec4<f32>,
    @location(5) row2: vec4<f32>,
    @location(6) normal0: vec4<f32>,
    @location(7) normal1: vec4<f32>,
    @location(8) normal2: vec4<f32>,
    @location(9) tint: vec4<f32>,
    @location(10) texture_bounds: vec4<f32>,
    @location(11) vertex_color: vec4<f32>,
    @location(12) material: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tint: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) texture_bounds: vec4<f32>,
    @location(5) @interpolate(flat) material: u32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let local = vec4<f32>(input.position, 1.0);
    output.world_position = vec3<f32>(
        dot(input.row0, local),
        dot(input.row1, local),
        dot(input.row2, local),
    );
    output.position = globals.view_projection * vec4<f32>(output.world_position, 1.0);
    output.normal = normalize(vec3<f32>(
        dot(input.normal0, vec4<f32>(input.normal, 0.0)),
        dot(input.normal1, vec4<f32>(input.normal, 0.0)),
        dot(input.normal2, vec4<f32>(input.normal, 0.0)),
    ));
    output.tint = input.tint * input.vertex_color;
    output.uv = input.uv;
    output.texture_bounds = input.texture_bounds;
    output.material = input.material;
    return output;
}

fn sample_terrain_layer(layer: i32, position: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let weights = pow(abs(normal), vec3<f32>(4.0));
    let normalized_weights = weights / max(weights.x + weights.y + weights.z, 0.0001);
    let scale = 0.18;
    let along_x = textureSampleLevel(terrain_textures, terrain_sampler, position.zy * scale, layer, 0.0).rgb;
    let along_y = textureSampleLevel(terrain_textures, terrain_sampler, position.xz * scale, layer, 0.0).rgb;
    let along_z = textureSampleLevel(terrain_textures, terrain_sampler, position.xy * scale, layer, 0.0).rgb;
    return along_x * normalized_weights.x + along_y * normalized_weights.y + along_z * normalized_weights.z;
}

fn material_texture(material: u32, position: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    if material == 7u {
        return sample_terrain_layer(0, position, normal);
    }
    if material == 1u {
        if normal.y < -0.72 {
            return sample_terrain_layer(2, position, normal);
        }
        let side = sample_terrain_layer(1, position, normal);
        let top = sample_terrain_layer(0, position, normal);
        return mix(side, top, smoothstep(0.35, 0.82, normal.y));
    }
    return sample_terrain_layer(i32(material), position, normal);
}

fn shadow_factor(world_position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let clip = shadow_globals.view_projection * vec4<f32>(world_position, 1.0);
    if clip.w <= 0.0001 { return 1.0; }
    let ndc = clip.xyz / clip.w;
    if ndc.z <= 0.0 || ndc.z >= 1.0 || abs(ndc.x) >= 1.0 || abs(ndc.y) >= 1.0 { return 1.0; }
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let light_direction = normalize(-globals.sun_direction.xyz);
    let depth = ndc.z - (0.0015 + (1.0 - max(dot(normal, light_direction), 0.0)) * 0.003);
    var visibility = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            visibility += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                uv + vec2<f32>(f32(x), f32(y)) * shadow_globals.texel_size.xy
                    * shadow_globals.texel_size.z,
                depth,
            );
        }
    }
    return visibility / 9.0;
}

@fragment
fn fs_main(input: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let normal = normalize(select(-input.normal, input.normal, front_facing));
    let view_direction = normalize(globals.camera_position.xyz - input.world_position);
    let light_direction = normalize(-globals.sun_direction.xyz);
    let direct = max(dot(normal, light_direction), 0.0);
    let shadow = shadow_factor(input.world_position, normal);
    let rim = pow(1.0 - max(dot(normal, view_direction), 0.0), 3.0) * 0.05;
    let lighting = dot(globals.lighting.xyz, vec3<f32>(0.33333334))
        + direct * 0.21 * globals.lighting.w * shadow;
    var albedo = vec3<f32>(1.0);
    if input.material != 0u {
        albedo = material_texture(input.material, input.world_position, normal);
    }
    if input.texture_bounds.z > 0.0 && input.texture_bounds.w > 0.0 {
        let texture_uv = input.texture_bounds.xy + input.uv * input.texture_bounds.zw;
        albedo = textureSample(world_texture, world_sampler, texture_uv).rgb;
    }
    var color = input.tint.rgb * albedo * lighting + vec3<f32>(rim);
    color *= globals.color_grade.x;
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, globals.color_grade.z);
    color = (color - vec3<f32>(0.5)) * globals.color_grade.y + vec3<f32>(0.5);
    let fog = smoothstep(
        globals.atmosphere.x,
        globals.atmosphere.y,
        distance(input.world_position, globals.camera_position.xyz),
    );
    return vec4<f32>(mix(color, globals.fog_color.rgb, fog), input.tint.a);
}
