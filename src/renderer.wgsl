struct Globals {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    sun_direction: vec4<f32>,
    fog_color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = globals.view_projection * vec4<f32>(input.position, 1.0);
    output.world_position = input.position;
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let view_direction = normalize(globals.camera_position.xyz - input.world_position);
    let light_direction = normalize(-globals.sun_direction.xyz);
    let direct_light = max(dot(normal, light_direction), 0.0);
    let lighting = 0.72 + direct_light * 0.42;
    let rim = pow(1.0 - max(dot(normal, view_direction), 0.0), 3.0) * 0.06;
    let lit_color = input.color.rgb * lighting + vec3<f32>(rim);
    let distance_to_camera = distance(input.world_position, globals.camera_position.xyz);
    let fog = smoothstep(52.0, 115.0, distance_to_camera);
    return vec4<f32>(mix(lit_color, globals.fog_color.rgb, fog), input.color.a);
}

struct UiVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_ui(input: VertexInput) -> UiVertexOutput {
    var output: UiVertexOutput;
    output.position = vec4<f32>(input.position.xy, 0.0, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_ui(input: UiVertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
