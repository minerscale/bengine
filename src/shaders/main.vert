#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_tex_coord;

layout(location = 0) out vec3 frag_normal;
layout(location = 1) out vec2 frag_tex_coord;
layout(location = 2) out float v_distance;

layout(constant_id = 0) const float front_clip = 0.0;
layout(constant_id = 1) const float back_clip = 0.0;

layout(set = 0, binding = 0) uniform View {
    float x;
    float y;
    float z;
    float rx;
    float ry;
    float rz;
    float rw;
    float time;
    float fov;
    float scale_y;
} view;

struct Isometry {
    float x;
    float y;
    float z;
    float rx;
    float ry;
    float rz;
    float rw;
};

struct MaterialProperties {
    float alpha_cutoff;
    bool is_water;
};

layout( push_constant ) uniform constants
{
    Isometry modelview;
    float alpha;
    MaterialProperties material_properties;
} push_constants;

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
    vec3 modelview_position = vec3(push_constants.modelview.x, push_constants.modelview.y, push_constants.modelview.z);
    vec4 modelview_rotor = vec4(push_constants.modelview.rx, push_constants.modelview.ry, push_constants.modelview.rz, push_constants.modelview.rw);

    vec3 rotated = rotate(in_position, modelview_rotor) + modelview_position;

    vec2 scale = -view.fov*vec2(1.0, view.scale_y);

    gl_Position = vec4(
        scale*rotated.xy,
        back_clip*(rotated.z - front_clip)/(back_clip - front_clip),
        rotated.z
    );

    v_distance = length(rotated);

    frag_normal = rotate(in_normal, modelview_rotor);
    frag_tex_coord = in_tex_coord;

    if (push_constants.material_properties.is_water) {
        // define triangle reference points in world space
        // You could pass these as uniforms if you want per-triangle control
        vec3 v0 = vec3(0.0, 0.0, 0.0);
        vec3 v1 = vec3(1.0, 0.0, 0.0);
        vec3 v2 = vec3(0.0, 0.0, 1.0);

        // triangle edges
        vec3 edge1 = v1 - v0;
        vec3 edge2 = v2 - v0;

        // vector from v0 to this vertex
        vec3 p = in_position - v0;

        // project onto edges to get UV coordinates
        float u = dot(p, edge1) / dot(edge1, edge1);
        float v = dot(p, edge2) / dot(edge2, edge2);

        frag_tex_coord = vec2(u, v);
    } else {
        frag_tex_coord = in_tex_coord;
    }
}
