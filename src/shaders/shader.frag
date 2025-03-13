#version 450

layout(location = 0) in vec3 frag_normal;
layout(location = 1) in vec2 frag_tex_coord;

layout(location = 0) out vec4 out_color;

layout(set = 1, binding = 1) uniform sampler2D tex_sampler;

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

layout(set = 0, binding = 0) uniform View {
    float x;
    float y;
    float z;
    float rx;
    float ry;
    float rz;
    float rw;
} view;

vec3 sun_color = vec3(1.0, 0.91, 0.56);
vec3 fill_light_color = vec3(0.5, 0.7, 0.9);

float shininess = 32.0;
float specular_reflection = 1.0;
float lambertian_reflection = 0.5;

vec3 lighting(vec3 view, vec3 light, vec3 color, vec3 normal) {
    float lambertian = max(dot(light, normalize(normal)), 0);
    float specular = pow(max(dot(reflect(-light, normal), view), 0), shininess);

    return color*vec3(lambertian*(specular_reflection*specular + lambertian_reflection));
}

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

vec3 rotor_to_vec(vec4 r) {
    return vec3(
        r.x*r.x + -r.y*r.y + -r.z*r.z + r.w*r.w,
        -2*r.x*r.y + -2*r.z*r.w,
        -2*r.x*r.z + 2*r.y*r.w
    );
}

void main() {
    vec3 normal = normalize(frag_normal);

    vec4 modelview_direction = vec4(modelview.rx, -modelview.ry, -modelview.rz, -modelview.rw);
    //vec4 view_direction = vec4(1.0, 0.0, 0.0, 0.0);
    vec3 sun = rotate(normalize(vec3(1.0, 1.0, 1.0)), vec4(view.rx, view.ry, view.rz, view.rw));
    vec3 view = rotor_to_vec(modelview_direction);

    //vec3 sun = normalize(-vec3(1.0, 1.0, 1.0));

    vec3 tex = texture(tex_sampler, vec2(frag_tex_coord.x, -frag_tex_coord.y)).xyz;
    out_color = vec4(tex * (
                    lighting(vec3(0.0,0.0,-1.0), sun, sun_color, normal) +
                    0.01 * fill_light_color +
                    0.1 * lighting(view, vec3(-sun.x,sun.y,-sun.z), fill_light_color, normal)
                ), 1.0);
}
