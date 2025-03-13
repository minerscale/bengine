#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_tex_coord;

layout(location = 0) out vec3 frag_normal;
layout(location = 1) out vec2 frag_tex_coord;

layout(constant_id = 0) const float scale_x = 0.0;
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
    float x = rotor.x;
    float y = rotor.y;
    float z = rotor.z;
    float w = rotor.w;
    
    vec4 q = vec4(
        dot(vec, vec3( x, y, z)),
        dot(vec, vec3(-y, x, w)),
        dot(vec, vec3(-z,-w, x)),
        dot(vec, vec3( w,-z, y))
    );

    return vec3(
        dot(q, vec4( x, y, z, w)),
        dot(q, vec4(-y, x, w,-z)),
        dot(q, vec4(-z,-w, x, y))
    );
}

/*vec3 rotate(vec3 v, vec4 r) {
    vec3 t = 2.0 * cross(r.xyz, v);
    return v + r.w * t + cross(r.xyz, t);
}*/

void main() {
    vec3 modelview_position = vec3(modelview.x, modelview.y, modelview.z);
    vec4 modelview_rotor = vec4(modelview.rx, modelview.ry, modelview.rz, modelview.rw);

    vec3 rotated = rotate(in_position, modelview_rotor) + modelview_position;

    vec2 scale = front_clip*vec2(scale_x, scale_y);

    gl_Position = vec4(scale*rotated.xy, back_clip*(rotated.z - front_clip)/(back_clip - front_clip), rotated.z);
    frag_normal = rotate(in_normal, vec4(modelview.rx, modelview.ry, modelview.rz, modelview.rw));
    frag_tex_coord = in_tex_coord;
}
