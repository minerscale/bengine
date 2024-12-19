#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec3 fragColor;
layout(location = 1) out vec2 fragTexCoord;

layout(constant_id = 0) const float scale_x = 0.0;
layout(constant_id = 1) const float scale_y = 0.0;
layout(constant_id = 2) const float front_clip = 0.0;
layout(constant_id = 3) const float back_clip = 0.0;

layout(binding = 0) uniform View {
    float x;
    float y;
    float z;
    float rx;
    float ry;
    float rz;
    float rw;
} view;

layout( push_constant ) uniform constants
{
    float x;
    float y;
    float z;
    float rx;
    float ry;
    float rz;
    float rw;
} model;

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

void main() {
    vec3 camera_position = vec3(view.x, view.y, view.z);
    vec4 camera_rotor = vec4(view.rx, view.ry, view.rz, view.rw);

    vec3 model_position = vec3(model.x, model.y, model.z);
    vec4 model_rotor = vec4(model.rx, model.ry, model.rz, model.rw);

    vec3 model_transformed = rotate(inPosition, model_rotor) + model_position;
    vec3 rotated = rotate(model_transformed - camera_position, camera_rotor);

    gl_Position = vec4(front_clip*vec2(scale_x, scale_y)*rotated.xy/rotated.z, (-rotated.z - front_clip)/back_clip, 1.0);

    fragColor = inColor;
    fragTexCoord = inTexCoord;
}
