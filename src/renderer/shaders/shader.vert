#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_tex_coord;

layout(location = 0) out vec3 frag_normal;
layout(location = 1) out vec2 frag_tex_coord;

layout(constant_id = 0) const float fov = 0.0;
layout(constant_id = 1) const float scale_y = 0.0;
layout(constant_id = 2) const float front_clip = 0.0;
layout(constant_id = 3) const float back_clip = 0.0;

layout( push_constant ) uniform constants
{
    float x;
    float y;
    float z;
    float rx;
    float ry;
    float rz;
    float rw;
} modelview;

vec3 rotate(vec3 vec, vec4 rotor) {
    float fx = rotor.x * vec.x + rotor.y * vec.y + rotor.z * vec.z;
    float fy = rotor.x * vec.y - rotor.y * vec.x + rotor.w * vec.z;
    float fz = rotor.x * vec.z - rotor.z * vec.x - rotor.w * vec.y;
    float fw = rotor.y * vec.z - rotor.z * vec.y + rotor.w * vec.x;
    
    return vec3(
        rotor.x * fx + rotor.y * fy + rotor.z * fz + rotor.w * fw,
        rotor.x * fy - rotor.y * fx - rotor.z * fw + rotor.w * fz,
        rotor.x * fz + rotor.y * fw - rotor.z * fx - rotor.w * fy
    );
}

void main() {
    vec3 modelview_position = vec3(modelview.x, modelview.y, modelview.z);
    vec4 modelview_rotor = vec4(modelview.rx, modelview.ry, modelview.rz, modelview.rw);

    vec3 rotated = rotate(in_position, modelview_rotor) + modelview_position;

    vec2 scale = -fov*vec2(1.0, scale_y);

    gl_Position = vec4(
        scale*rotated.xy,
        back_clip*(rotated.z - front_clip)/(back_clip - front_clip),
        rotated.z
    );

    frag_normal = rotate(in_normal, modelview_rotor);
    frag_tex_coord = in_tex_coord;
}
