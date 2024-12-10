#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

layout(constant_id = 0) const float scale_x = 0.0;
layout(constant_id = 1) const float scale_y = 0.0;
layout(constant_id = 2) const float scale_z = 0.0;

layout( push_constant ) uniform constants
{
    vec4 camera_rotation;
    vec3 camera_position;
} PushConstants;

vec3 rotate(vec3 vec, vec4 rotor) {
    float x = rotor.x;
    float y = rotor.y;
    float z = rotor.z;
    float w = rotor.w;

    vec3 v = vec3(vec.x, vec.y, vec.z);

    vec4 q = vec4(
        dot(v, vec3( x, y, z)),
        dot(v, vec3(-y, x, w)),
        dot(v, vec3(-z,-w, x)),
        dot(v, vec3( w,-z, y))
    );

    return vec3(
        dot(q, vec4( x, y, z, w)),
        dot(q, vec4(-y, x, w,-z)),
        dot(q, vec4(-z,-w, x, y))
    );
}

void main() {
    vec3 rotated = rotate(inPosition - PushConstants.camera_position, PushConstants.camera_rotation);
    gl_Position = vec4(vec2(scale_x, scale_y)*rotated.xy/rotated.z, -scale_z*rotated.z - 0.01, 1.0);

    fragColor = inColor;
}
