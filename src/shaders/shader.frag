#version 450

layout(location = 0) in vec3 fragColor;

layout(location = 0) out vec4 outColor;

layout( push_constant ) uniform constants
{
    layout(offset = 64) vec3 sun_direction;
} PushConstants;

vec3 sun_color = vec3(1.0, 0.91, 0.56);

vec3 lighting(vec3 light, vec3 color) {
    return color*vec3(max(dot(light, normalize(fragColor)),0));
}

void main() {
    vec3 sun = PushConstants.sun_direction;

    outColor = vec4(vec3(0.4, 0.7, 0.9) * (
                    lighting(sun, sun_color) +
                    0.01 * vec3(0.5, 0.7, 0.9) +
                    0.2 * lighting(vec3(-sun.x,sun.y,-sun.z), vec3(0.5, 0.7, 0.9))
                ), 1.0);
}
