#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

layout(constant_id = 0) const float scale_x = 0.0;
layout(constant_id = 1) const float scale_y = 0.0;
layout(constant_id = 2) const float front_clip = 0.0;
layout(constant_id = 3) const float back_clip = 0.0;

layout( push_constant ) uniform constants
{
    float cx;
    float cy;
    float cz;
    float crx;
    float cry;
    float crz;
    float crw;
    float mx;
    float my;
    float mz;
    float mrx;
    float mry;
    float mrz;
    float mrw;
} PushConstants;

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
    vec3 camera_position = vec3(PushConstants.cx, PushConstants.cy, PushConstants.cz);
    vec4 camera_rotor = vec4(PushConstants.crx, PushConstants.cry, PushConstants.crz, PushConstants.crw);

    vec3 model_position = vec3(PushConstants.mx, PushConstants.my, PushConstants.mz);
    vec4 model_rotor = vec4(PushConstants.mrx, PushConstants.mry, PushConstants.mrz, PushConstants.mrw);

    vec3 model_transformed = rotate(inPosition, model_rotor) + model_position;
    vec3 rotated = rotate(model_transformed - camera_position, camera_rotor);

    gl_Position = vec4(front_clip*vec2(scale_x, scale_y)*rotated.xy/rotated.z, (-rotated.z - front_clip)/back_clip, 1.0);

    fragColor = inColor;
}
