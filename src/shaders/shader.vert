#version 450

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;

layout(location = 0) out vec3 fragColor;

layout( push_constant ) uniform constants
{
    vec4 camera_rotation;
    vec3 camera_position;
    float align;
    vec3 camera_scale;
} PushConstants;

vec3 rotate(vec3 vec, vec4 rotor) {
    float k = rotor.x;
    float a = rotor.y;
    float b = rotor.z;
    float c = rotor.w;

    float x = vec.x;
    float y = vec.y;
    float z = vec.z;

    float r = k * x + a * y - b * z;
    float s = -a * x + k * y + c * z;
    float t = b * x + k * z - c * y;
    float u = c * x + b * y + a * z;

    return vec3(
        r * k + s * a - t * b + u * c,
        -r * a + s * k + t * c + u * b,
        r * b - s * c + t * k + u * a
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
    vec3 transformed = -PushConstants.camera_scale*rotated;
    gl_Position = vec4(transformed, 1.0);

    vec4 reverse = vec4(PushConstants.camera_rotation.x, -PushConstants.camera_rotation.yzw);
    vec3 sunlight = normalize(rotate(sun, reverse));
    vec3 backlight = normalize(rotate(back, reverse));
    fragColor = teapot_color*(0.9*lighting(sunlight, sun_color) + 0.005*lighting(backlight, back_color));
}
