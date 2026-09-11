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
    @location(6) occlusion: f32,
};
fn decode_srgb(v: vec3<f32>) -> vec3<f32> {
    return select(pow((v + 0.055) / 1.055, vec3<f32>(2.4)), v / 12.92, v <= vec3<f32>(0.04045));
}
fn encode_srgb(v: vec3<f32>) -> vec3<f32> {
    let c = max(v, vec3<f32>(0.0));
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}
fn turn_vector(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    return v*cos(angle) + cross(axis,v)*sin(angle) + axis*dot(axis,v)*(1.0-cos(angle));
}
@vertex fn vs_main(input: Input) -> Output {
    var output: Output;
    var position = input.position;
    var normal = input.normal;
    if input.material.w == 17.0 {
        // One continuous sleeve, weighted around the existing elbow. The three
        // normal-row w lanes carry axis-angle; no new attributes/bones/uploads.
        // Dimensions/pivot match hero_character::{SLEEVE_SIZE,SLEEVE_CENTER,ELBOW}.
        let size = vec3<f32>(0.46,1.13,0.49);
        let pivot = vec3<f32>(0.0,-0.12,0.0);
        let turn = vec3<f32>(input.normal0.w,input.normal1.w,input.normal2.w);
        let angle = length(turn);
        let axis = turn / max(angle,0.00001);
        let physical = position*size;
        let f = clamp((physical.y+0.22)/0.24,0.0,1.0);
        let weight = 1.0-f*f*(3.0-2.0*f);
        let delta = turn_vector(physical-pivot,axis,angle)-(physical-pivot);
        position = (physical+weight*delta)/size;
        // Inverse-transpose of the deformation Jacobian, including the weight
        // gradient. This keeps the bent cloth shading continuous at the elbow.
        let x = mix(vec3<f32>(1,0,0),turn_vector(vec3<f32>(1,0,0),axis,angle),weight);
        let y = mix(vec3<f32>(0,1,0),turn_vector(vec3<f32>(0,1,0),axis,angle),weight)
            + delta*(-6.0*f*(1.0-f)/0.24);
        let z = mix(vec3<f32>(0,0,1),turn_vector(vec3<f32>(0,0,1),axis,angle),weight);
        normal = (mat3x3<f32>(cross(y,z),cross(z,x),cross(x,y))*(normal/size))*size;
    }
    let p = vec4<f32>(position, 1.0);
    output.world = vec3<f32>(dot(input.row0, p), dot(input.row1, p), dot(input.row2, p));
    output.position = globals.view_projection * vec4<f32>(output.world, 1.0);
    output.normal = vec3<f32>(dot(input.normal0.xyz, normal), dot(input.normal1.xyz, normal), dot(input.normal2.xyz, normal));
    output.tint = input.tint;
    output.material = input.material;
    output.uv = input.uv;
    output.local_normal_z = input.normal.z;
    output.occlusion = 1.0;
    if input.material.w == 12.0 {
        // The hood is hollow geometry; its inward-facing cloth receives less
        // environment light than the outside of the rolled rim.
        output.occlusion = 0.55 + 0.45 * smoothstep(-0.12, 0.22,
            dot(input.position.xz, input.normal.xz));
    } else if input.material.w == 8.0 {
        output.occlusion = 1.0 - smoothstep(0.72, 1.0, input.uv.y) * 0.18;
    }
    return output;
}
@fragment fn fs_main(input: Output) -> @location(0) vec4<f32> {
    // Derivatives must be evaluated before material branches/discards for
    // WebGPU's uniform-control-flow contract (also valid on Metal and GL).
    let uv_footprint = max(fwidth(input.uv.x), fwidth(input.uv.y));
    // Ordered coverage fade preserves depth and avoids sorting the many rigid
    // pieces as the local camera enters the head. No extra transparency pass.
    if input.tint.a < 1.0 && input.material.z <= 0.0 {
        let cell = vec2<u32>(input.position.xy) % vec2<u32>(4u);
        let bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
        if input.tint.a <= (bayer[cell.y * 4u + cell.x] + 0.5) / 16.0 { discard; }
    }
    // Analytic graphic faces keep curved eyes and smiles crisp at close range
    // without extra meshes or a face texture. Discard preserves occlusion.
    if (input.material.w >= 4.0 && input.material.w <= 7.0) || input.material.w == 9.0 || input.material.w == 10.0 || input.material.w == 18.0 {
        if input.local_normal_z > -0.9 { discard; }
        let p = input.uv * 2.0 - 1.0;
        var color = input.tint.rgb;
        var coverage=1.0;
        if input.material.w == 18.0 {
            // A single tapered stroke continues the main smile around either
            // cheek. The side lane selects its slope; unlike a second mouth,
            // this stays visually connected in front and profile views.
            let side = input.material.x;
            let curve = input.material.y;
            let centerline = curve * (0.27 + side * p.x * 0.16);
            let stroke_width = 0.13 * sqrt(max(0.0, 1.0 - p.x * p.x));
            let stroke = stroke_width - abs(p.y - centerline);
            let end = 1.0 - abs(p.x);
            let edge = min(stroke, end);
            let aa = max(uv_footprint * 1.5, 0.006);
            if edge <= -aa { discard; }
            coverage = smoothstep(-aa, aa, edge);
            color = vec3<f32>(0.13, 0.085, 0.065);
        } else if input.material.w == 9.0 {
            let radius = dot(p,p);
            if radius > 0.94 { discard; }
            coverage=1.0-smoothstep(0.94-max(uv_footprint*4.0,0.008),0.94,radius);
            // Negative z packs eyelid opening for the hero's graphic face.
            // A closing eye becomes a lid stroke, rather than a tiny iris.
            if input.material.z > -0.30 {
                color=vec3<f32>(0.16,0.09,0.055);
                if abs(p.y-0.25*(1.0-p.x*p.x))>0.30 { discard; }
            } else {
                // Keep the hero/starter path consistent with regular person
                // eyes: warm sclera plus an iris, pupil, and catchlight gives
                // every skin tone the same clear focal point.
                let rim = smoothstep(0.78, 0.94, radius);
                color = mix(vec3<f32>(0.93, 0.89, 0.82), vec3<f32>(0.12, 0.07, 0.045), rim);
                let iris_center = vec2<f32>(0.02, -0.01);
                let iris_p = (p - iris_center) / vec2<f32>(0.30, 0.44);
                if dot(iris_p, iris_p) < 1.0 {
                    color = vec3<f32>(0.17, 0.095, 0.055);
                    let pupil_p = (p - iris_center) / vec2<f32>(0.12, 0.23);
                    if dot(pupil_p, pupil_p) < 1.0 {
                        color = vec3<f32>(0.018, 0.012, 0.010);
                    }
                    let catchlight_p = (p - (iris_center + vec2<f32>(-0.09, 0.16)))
                        / vec2<f32>(0.055, 0.080);
                    if dot(catchlight_p, catchlight_p) < 1.0 {
                        color = vec3<f32>(0.99, 0.98, 0.94);
                    }
                }
            }
        } else if input.material.w == 10.0 {
            let arch=0.38*(1.0-p.x*p.x)-0.15;
            let edge=abs(p.y-arch)-0.28*sqrt(max(0.0,1.0-p.x*p.x));
            if abs(p.x)>0.96 || edge>0.0 { discard; }
            coverage=1.0-smoothstep(-max(uv_footprint*2.0,0.006),0.0,edge);
        } else if input.material.w == 4.0 {
            // Person eyes use the same readable graphic treatment at every
            // skin tone: a softly warm sclera, dark iris/pupil, and a small
            // catchlight. Keeping this palette independent of the face tint
            // prevents very dark skin from swallowing the old black ovals.
            let eye_p = p / vec2<f32>(0.96, 0.94);
            let eye_radius = dot(eye_p, eye_p);
            if eye_radius > 1.0 { discard; }
            let rim = smoothstep(0.78, 1.0, eye_radius);
            let sclera = vec3<f32>(0.93, 0.89, 0.82);
            let outline = vec3<f32>(0.12, 0.07, 0.045);
            color = mix(sclera, outline, rim);

            let iris_center = vec2<f32>(0.02, -0.01);
            let iris_p = (p - iris_center) / vec2<f32>(0.30, 0.44);
            if dot(iris_p, iris_p) < 1.0 {
                color = vec3<f32>(0.17, 0.095, 0.055);
                let pupil_p = (p - iris_center) / vec2<f32>(0.12, 0.23);
                if dot(pupil_p, pupil_p) < 1.0 {
                    color = vec3<f32>(0.018, 0.012, 0.010);
                }
                let catchlight_p = (p - (iris_center + vec2<f32>(-0.09, 0.16)))
                    / vec2<f32>(0.055, 0.080);
                if dot(catchlight_p, catchlight_p) < 1.0 {
                    color = vec3<f32>(0.99, 0.98, 0.94);
                }
            }
        } else if input.material.w == 5.0 {
            let curve = input.material.x;
            let opening = clamp(input.material.y, 0.0, 1.0);
            let x = p.x / 0.88;
            let smile = curve * (x * x - 0.45) * (0.75 - opening * 0.35);
            let edge = sqrt(max(0.0, 1.0 - x * x));
            let upper = smile + (0.11 + opening * 0.45) * edge;
            let lower = smile - (0.11 + opening * 0.65) * edge;
            // One continuous mouth, bounded inside the face quad even at full
            // opening and curvature. Use the same derivative footprint for
            // the side and curved lip boundaries so small mouths do not turn
            // into jagged binary pixels at gameplay distance.
            let side_edge = 1.0 - abs(x);
            let lip_edge = min(upper - p.y, p.y - lower);
            let mouth_edge = min(side_edge, lip_edge);
            let mouth_aa = max(uv_footprint * 1.5, 0.002);
            if mouth_edge <= -mouth_aa { discard; }
            coverage = smoothstep(-mouth_aa, mouth_aa, mouth_edge);
            if opening > 0.12 {
                color = vec3<f32>(0.08, 0.05, 0.07);
                // A tiny tooth or tongue cue gives the open expressions a
                // stronger read without adding another mesh or texture.
                if opening > 0.38 && p.y > upper - 0.20 {
                    color = vec3<f32>(0.96, 0.92, 0.82);
                } else if opening > 0.30 && p.y < lower + 0.18 {
                    color = vec3<f32>(0.82, 0.29, 0.34);
                }
            }
        } else if input.material.w == 6.0 {
            if dot(p, p) > 0.92 { discard; }
        } else {
            if dot(p, p) > 0.94 { discard; }
            color *= 0.92;
        }
        let fog = smoothstep(100.0, 220.0, distance(input.world, globals.camera_position.xyz));
        return vec4<f32>(mix(color, globals.fog_color.rgb, fog), coverage);
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
    if input.material.w >= 8.0 {
        let wrapped = max((dot(normal,light)+0.18)/1.18,0.0);
        let sky = mix(vec3<f32>(0.18,0.22,0.28),vec3<f32>(0.43,0.52,0.62),normal.y*0.5+0.5);
        lit = base * (sky + vec3<f32>(1.0,0.89,0.73)*wrapped*0.92) * input.occlusion
            + vec3<f32>(1.0,0.94,0.84)*specular + base*rim*1.4;
        if input.material.w == 11.0 {
            lit += base*vec3<f32>(0.14,0.055,0.025)*pow(1.0-abs(dot(normal,light)),3.0);
        }
    }
    if input.material.w == 14.0 {
        let slot = min(abs(input.uv.x-0.605),abs(input.uv.x-0.895));
        let edge = 1.0-smoothstep(0.009,0.009+max(uv_footprint,0.004),slot);
        let height = smoothstep(0.32,0.44,input.uv.y)*(1.0-smoothstep(0.84,0.94,input.uv.y));
        lit *= 1.0-edge*height*0.48;
    }
    if input.material.w == 16.0 {
        let visibility=1.0-smoothstep(0.35,1.0,uv_footprint*48.0);
        lit *= 0.94+sin(input.uv.x*301.5929)*0.075*visibility;
    }
    if input.material.w == 13.0 {
        // Broad, filtered comb marks support the sculpted locks. Their warm
        // highlights stay tied to lighting instead of painted white stripes.
        let visibility = 1.0-smoothstep(0.22,0.85,uv_footprint*18.0);
        let comb = 0.5+0.5*cos(input.uv.x*37.6991+input.uv.y*3.0);
        lit *= 1.0-(1.0-comb)*0.10*visibility;
        lit += base*vec3<f32>(0.20,0.12,0.065)
            *pow(max(dot(normal,half_vector),0.0),18.0)*visibility;
    }
    if input.material.w == 1.0 || input.material.w == 8.0 || input.material.w == 12.0 || input.material.w == 14.0 || input.material.w == 17.0 {
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
