struct CursorUniform {
    pos: vec2<f32>,
    size: vec2<f32>,
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
};

@binding(0) @group(0)
var<uniform> cursor: CursorUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    let pixel_pos = (position - 0.5) * cursor.size + cursor.pos;
    let screen_pos = pixel_pos / cursor.screen_size;
    let ndc = screen_pos * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, ndc.y, 0.0, 1.0);
    out.uv = position;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.95, 0.2, 1.0);
}
