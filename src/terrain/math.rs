pub(crate) fn trilinear(values: [f32; 8], point: [f32; 3]) -> f32 {
    let [x, y, z] = point;
    let x00 = values[0] + (values[1] - values[0]) * x;
    let x10 = values[2] + (values[3] - values[2]) * x;
    let x01 = values[4] + (values[5] - values[4]) * x;
    let x11 = values[6] + (values[7] - values[6]) * x;
    let y0 = x00 + (x10 - x00) * y;
    let y1 = x01 + (x11 - x01) * y;
    y0 + (y1 - y0) * z
}

pub(crate) fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let length = dot(vector, vector).sqrt();
    if length <= 1.0e-6 {
        [0.0, 1.0, 0.0]
    } else {
        vector.map(|value| value / length)
    }
}

pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(crate) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
