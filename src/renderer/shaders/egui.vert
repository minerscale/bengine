// -----------------------------------------------
// Adapted from
// https://github.com/emilk/egui/blob/main/crates/egui_glow/src/shader/vertex.glsl
#version 450

layout(location = 0) in vec2 pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;

layout(location = 0) out vec4 out_color;
layout(location = 1) out vec2 out_uv;

layout(constant_id = 0) const float screen_size_x = 0.0;
layout(constant_id = 1) const float screen_size_y = 0.0;

layout( push_constant ) uniform constants
{
    float pixels_per_point;
};

void main() {
    gl_Position = vec4(
        (2.0 * pos.x) / (screen_size_x / pixels_per_point) - 1.0,
        (2.0 * pos.y) / (screen_size_y / pixels_per_point) - 1.0,
        0.0,
        1.0
    );

    //out_color = vec4(in_color >> 24, (in_color >> 16) & 0xFF, (in_color >> 8) & 0xFF, in_color & 0xFF) / 255.0;
    out_color = in_color;
    out_uv = in_uv;
}
