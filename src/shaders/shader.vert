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
    float align;
} PushConstants;

vec3 rotate(vec3 vec, vec4 rotor) {
    float x = rotor.x;
    float y = rotor.y;
    float z = rotor.z;
    float w = rotor.w;

    vec4 q = vec4(
        dot(vec, vec3( x, y,-z)),
        dot(vec, vec3(-y, x, w)),
        dot(vec, vec3( z,-w, x)),
        dot(vec, vec3( w, z, y))
    );

    return vec3(
        dot(q, vec4(x, y,-z, w)),
        dot(q, vec4(-y,x, w, z)),
        dot(q, vec4(z,-w, x, y))
    );
}

float root3 = 1.0/sqrt(3.0);
vec3 sun = vec3(-root3, root3, root3);
vec3 sun_color = vec3(1.0, 0.91, 0.56);

float root2 = 1.0/sqrt(2.0);
vec3 back = vec3(1, 0, 0);
vec3 back_color = vec3(0.698, 0.9, 1.0);

vec3 teapot_color = vec3(0.4, 0.7, 0.9);

vec3 lighting(vec3 light, vec3 color) {
    return color*vec3(max(dot(light, inColor),0));
}

void main() {
    vec3 rotated = rotate(inPosition - PushConstants.camera_position, PushConstants.camera_rotation);
    gl_Position = vec4(vec2(scale_x, scale_y)*rotated.xy/rotated.z, -scale_z*rotated.z-0.1, 1.0);

    //vec4 reverse = vec4(PushConstants.camera_rotation.x, -PushConstants.camera_rotation.yzw);
    //vec3 sunlight = normalize(rotate(sun, reverse));
    //vec3 backlight = normalize(rotate(back, reverse));
    //fragColor = teapot_color*(0.9*lighting(sunlight, sun_color) + 0.005*lighting(backlight, back_color));
    fragColor = inColor;
}
