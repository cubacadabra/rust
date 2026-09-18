struct Globals {
    view_projection: mat4x4<f32>,
    sun_direction_brightness: vec4<f32>,
    ambient: vec4<f32>,
    color_correction: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = globals.view_projection * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let sun = normalize(globals.sun_direction_brightness.xyz);
    let diffuse = max(dot(normal, sun), 0.0);
    let directional = diffuse * globals.sun_direction_brightness.w * 0.42;
    var color = input.color.rgb * (globals.ambient.rgb + vec3<f32>(directional));

    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    color = mix(vec3<f32>(luminance), color, 1.0 + globals.color_correction.z);
    color = (color - vec3<f32>(0.5)) * (1.0 + globals.color_correction.y)
        + vec3<f32>(0.5 + globals.color_correction.x);
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
