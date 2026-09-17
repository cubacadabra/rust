struct Globals {
    view_projection: mat4x4<f32>,
    camera_position: vec4<f32>,
    sun_direction: vec4<f32>,
    fog_color: vec4<f32>,
    color_grade: vec4<f32>,
    atmosphere: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

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
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tint: vec4<f32>,
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
    output.tint = input.tint;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(input.normal);
    let view_direction = normalize(globals.camera_position.xyz - input.world_position);
    let light_direction = normalize(-globals.sun_direction.xyz);
    let direct = max(dot(normal, light_direction), 0.0);
    let lighting = 0.72 + direct * 0.42;
    let rim = pow(1.0 - max(dot(normal, view_direction), 0.0), 3.0) * 0.05;
    var color = input.tint.rgb * lighting + vec3<f32>(rim);
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
