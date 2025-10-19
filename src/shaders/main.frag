#version 450

layout(location = 0) in vec3 frag_normal;
layout(location = 1) in vec2 frag_tex_coord;
layout(location = 2) in float v_distance;

layout(location = 0) out vec4 out_color;

layout(set = 1, binding = 0) uniform sampler2D tex_sampler;

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

vec3 sun_color = vec3( 0.7, 0.75, 1.0 );
vec3 fill_light_color = vec3( 0.1, 0.6, 1.0 );

float shininess = 2.0;
float specular_reflection = 0.05;
float lambertian_reflection = 1.0;

vec3 lighting(vec3 view, vec3 light, vec3 color, vec3 normal) {
    float lambertian = max(dot(light, normalize(normal)), 0);
    float specular = pow(max(dot(reflect(-light, normal), view), 0), shininess);

    return color*vec3(lambertian*(specular_reflection*specular + lambertian_reflection));
}

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

vec3 rotor_to_vec(vec4 r) {
    return vec3(
        r.x*r.x + -r.y*r.y + -r.z*r.z + r.w*r.w,
        -2*r.x*r.y + -2*r.z*r.w,
        -2*r.x*r.z + 2*r.y*r.w
    );
}

// from http://www.java-gaming.org/index.php?topic=35123.0
vec4 cubic(float v){
    vec4 n = vec4(1.0, 2.0, 3.0, 4.0) - v;
    vec4 s = n * n * n;
    float x = s.x;
    float y = s.y - 4.0 * s.x;
    float z = s.z - 4.0 * s.y + 6.0 * s.x;
    float w = 6.0 - x - y - z;
    return vec4(x, y, z, w) * (1.0/6.0);
}

vec4 textureBicubic(sampler2D tex, vec2 texCoords){
   vec2 texSize = textureSize(tex, 0);
   vec2 invTexSize = 1.0 / texSize;
   
   texCoords = texCoords * texSize - 0.5;

   
    vec2 fxy = fract(texCoords);
    texCoords -= fxy;

    vec4 xcubic = cubic(fxy.x);
    vec4 ycubic = cubic(fxy.y);

    vec4 c = texCoords.xxyy + vec2 (-0.5, +1.5).xyxy;
    
    vec4 s = vec4(xcubic.xz + xcubic.yw, ycubic.xz + ycubic.yw);
    vec4 offset = c + vec4 (xcubic.yw, ycubic.yw) / s;
    
    offset *= invTexSize.xxyy;
    
    vec4 sample0 = texture(tex, offset.xz);
    vec4 sample1 = texture(tex, offset.yz);
    vec4 sample2 = texture(tex, offset.xw);
    vec4 sample3 = texture(tex, offset.yw);

    float sx = s.x / (s.x + s.y);
    float sy = s.z / (s.z + s.w);

    return mix(
       mix(sample3, sample2, sx), mix(sample1, sample0, sx)
    , sy);
}

// "Wind Waker Ocean" by @Polyflare (29/1/15)
// License: Creative Commons Attribution 4.0 International

// Source code for the texture generator is available at:
// https://github.com/lmurray/circleator

//-----------------------------------------------------------------------------
// User settings

// 0 = No antialiasing
// 1 = 2x2 supersampling antialiasing
#define ANTIALIAS 0
#define MAX_SAMPLES 4.0

// 0 = Do not distort the water texture
// 1 = Apply lateral distortion to the water texture
#define DISTORT_WATER 1

// 0 = Disable parallax effects
// 1 = Change the height of the water with parallax effects
#define PARALLAX_WATER 1

// 0 = Antialias the water texture
// 1 = Do not antialias the water texture
#define FAST_CIRCLES 1

//-----------------------------------------------------------------------------

#define WATER_COL vec3(0.0, 0.4453, 0.7305)
#define WATER2_COL vec3(0.0, 0.4180, 0.6758)
#define FOAM_COL vec3(0.8125, 0.9609, 0.9648)
#define FOG_COL vec3(0.6406, 0.9453, 0.9336)
#define SKY_COL vec3(0.0, 0.8203, 1.0)

#define M_2PI 6.283185307
#define M_6PI 18.84955592

float circ(vec2 pos, vec2 c, float s)
{
    c = abs(pos - c);
    c = min(c, 1.0 - c);
#if FAST_CIRCLES
    return dot(c, c) < s ? -1.0 : 0.0;
#else
    return smoothstep(0.0, 0.002, sqrt(s) - sqrt(dot(c, c))) * -1.0;
#endif
}

// Foam pattern for the water constructed out of a series of circles
float waterlayer(vec2 uv)
{
    uv = mod(uv, 1.0); // Clamp to [0..1]
    float ret = 1.0;
    ret += circ(uv, vec2(0.37378, 0.277169), 0.0268181);
    ret += circ(uv, vec2(0.0317477, 0.540372), 0.0193742);
    ret += circ(uv, vec2(0.430044, 0.882218), 0.0232337);
    ret += circ(uv, vec2(0.641033, 0.695106), 0.0117864);
    ret += circ(uv, vec2(0.0146398, 0.0791346), 0.0299458);
    ret += circ(uv, vec2(0.43871, 0.394445), 0.0289087);
    ret += circ(uv, vec2(0.909446, 0.878141), 0.028466);
    ret += circ(uv, vec2(0.310149, 0.686637), 0.0128496);
    ret += circ(uv, vec2(0.928617, 0.195986), 0.0152041);
    ret += circ(uv, vec2(0.0438506, 0.868153), 0.0268601);
    ret += circ(uv, vec2(0.308619, 0.194937), 0.00806102);
    ret += circ(uv, vec2(0.349922, 0.449714), 0.00928667);
    ret += circ(uv, vec2(0.0449556, 0.953415), 0.023126);
    ret += circ(uv, vec2(0.117761, 0.503309), 0.0151272);
    ret += circ(uv, vec2(0.563517, 0.244991), 0.0292322);
    ret += circ(uv, vec2(0.566936, 0.954457), 0.00981141);
    ret += circ(uv, vec2(0.0489944, 0.200931), 0.0178746);
    ret += circ(uv, vec2(0.569297, 0.624893), 0.0132408);
    ret += circ(uv, vec2(0.298347, 0.710972), 0.0114426);
    ret += circ(uv, vec2(0.878141, 0.771279), 0.00322719);
    ret += circ(uv, vec2(0.150995, 0.376221), 0.00216157);
    ret += circ(uv, vec2(0.119673, 0.541984), 0.0124621);
    ret += circ(uv, vec2(0.629598, 0.295629), 0.0198736);
    ret += circ(uv, vec2(0.334357, 0.266278), 0.0187145);
    ret += circ(uv, vec2(0.918044, 0.968163), 0.0182928);
    ret += circ(uv, vec2(0.965445, 0.505026), 0.006348);
    ret += circ(uv, vec2(0.514847, 0.865444), 0.00623523);
    ret += circ(uv, vec2(0.710575, 0.0415131), 0.00322689);
    ret += circ(uv, vec2(0.71403, 0.576945), 0.0215641);
    ret += circ(uv, vec2(0.748873, 0.413325), 0.0110795);
    ret += circ(uv, vec2(0.0623365, 0.896713), 0.0236203);
    ret += circ(uv, vec2(0.980482, 0.473849), 0.00573439);
    ret += circ(uv, vec2(0.647463, 0.654349), 0.0188713);
    ret += circ(uv, vec2(0.651406, 0.981297), 0.00710875);
    ret += circ(uv, vec2(0.428928, 0.382426), 0.0298806);
    ret += circ(uv, vec2(0.811545, 0.62568), 0.00265539);
    ret += circ(uv, vec2(0.400787, 0.74162), 0.00486609);
    ret += circ(uv, vec2(0.331283, 0.418536), 0.00598028);
    ret += circ(uv, vec2(0.894762, 0.0657997), 0.00760375);
    ret += circ(uv, vec2(0.525104, 0.572233), 0.0141796);
    ret += circ(uv, vec2(0.431526, 0.911372), 0.0213234);
    ret += circ(uv, vec2(0.658212, 0.910553), 0.000741023);
    ret += circ(uv, vec2(0.514523, 0.243263), 0.0270685);
    ret += circ(uv, vec2(0.0249494, 0.252872), 0.00876653);
    ret += circ(uv, vec2(0.502214, 0.47269), 0.0234534);
    ret += circ(uv, vec2(0.693271, 0.431469), 0.0246533);
    ret += circ(uv, vec2(0.415, 0.884418), 0.0271696);
    ret += circ(uv, vec2(0.149073, 0.41204), 0.00497198);
    ret += circ(uv, vec2(0.533816, 0.897634), 0.00650833);
    ret += circ(uv, vec2(0.0409132, 0.83406), 0.0191398);
    ret += circ(uv, vec2(0.638585, 0.646019), 0.0206129);
    ret += circ(uv, vec2(0.660342, 0.966541), 0.0053511);
    ret += circ(uv, vec2(0.513783, 0.142233), 0.00471653);
    ret += circ(uv, vec2(0.124305, 0.644263), 0.00116724);
    ret += circ(uv, vec2(0.99871, 0.583864), 0.0107329);
    ret += circ(uv, vec2(0.894879, 0.233289), 0.00667092);
    ret += circ(uv, vec2(0.246286, 0.682766), 0.00411623);
    ret += circ(uv, vec2(0.0761895, 0.16327), 0.0145935);
    ret += circ(uv, vec2(0.949386, 0.802936), 0.0100873);
    ret += circ(uv, vec2(0.480122, 0.196554), 0.0110185);
    ret += circ(uv, vec2(0.896854, 0.803707), 0.013969);
    ret += circ(uv, vec2(0.292865, 0.762973), 0.00566413);
    ret += circ(uv, vec2(0.0995585, 0.117457), 0.00869407);
    ret += circ(uv, vec2(0.377713, 0.00335442), 0.0063147);
    ret += circ(uv, vec2(0.506365, 0.531118), 0.0144016);
    ret += circ(uv, vec2(0.408806, 0.894771), 0.0243923);
    ret += circ(uv, vec2(0.143579, 0.85138), 0.00418529);
    ret += circ(uv, vec2(0.0902811, 0.181775), 0.0108896);
    ret += circ(uv, vec2(0.780695, 0.394644), 0.00475475);
    ret += circ(uv, vec2(0.298036, 0.625531), 0.00325285);
    ret += circ(uv, vec2(0.218423, 0.714537), 0.00157212);
    ret += circ(uv, vec2(0.658836, 0.159556), 0.00225897);
    ret += circ(uv, vec2(0.987324, 0.146545), 0.0288391);
    ret += circ(uv, vec2(0.222646, 0.251694), 0.00092276);
    ret += circ(uv, vec2(0.159826, 0.528063), 0.00605293);
    return max(ret, 0.0);
}

// Procedural texture generation for the water
vec3 water(vec2 uv, vec3 cdir, float time)
{
    //uv *= vec2(0.25);
    
#if PARALLAX_WATER
    // Parallax height distortion with two directional waves at
    // slightly different angles.
    vec2 a = 0.025 * cdir.xz / cdir.y; // Parallax offset
    float h = sin(uv.x + time); // Height at UV
    uv += a * h;
    h = sin(0.841471 * uv.x - 0.540302 * uv.y + time);
    uv += a * h;
#endif

#if DISTORT_WATER
    // Texture distortion
    float d1 = mod(uv.x + uv.y, M_2PI);
    float d2 = mod((uv.x + uv.y + 0.25) * 1.3, M_6PI);
    d1 = time * 0.07 + d1;
    d2 = time * 0.5 + d2;
    vec2 dist = vec2(
        sin(d1) * 0.15 + sin(d2) * 0.05,
        cos(d1) * 0.15 + cos(d2) * 0.05
    );
#else
    const vec2 dist = vec2(0.0);
#endif
    
    vec3 ret = mix(WATER_COL, WATER2_COL, waterlayer(uv + dist.xy));
    ret = mix(ret, FOAM_COL, waterlayer(vec2(1.0) - uv - dist.yx));
    return ret;
}



void main() {
    if (push_constants.material_properties.is_water) {
    vec2 uv = 0.25 * frag_tex_coord;

    vec2 ddx = dFdx(uv);
    vec2 ddy = dFdy(uv);
    float pixelFootprint = 0.5 * (length(ddx) + length(ddy));

    float dist = v_distance;

    float t = clamp(dist / 25.0, 0.0, 1.0);
    int samples = int(mix(3.0, MAX_SAMPLES, t));

    if (samples <= 1) {
        out_color = vec4(water(uv, vec3(1.0), view.time), 1.0);
    } else {
        samples = min(samples, 16);

        const vec2 OFFSETS_16[16] = vec2[](
            vec2(-0.613392,  0.617481),
            vec2( 0.170019, -0.040254),
            vec2(-0.299417,  0.791925),
            vec2( 0.645680,  0.493210),
            vec2(-0.651784,  0.717887),
            vec2( 0.421003,  0.027070),
            vec2(-0.817194, -0.271096),
            vec2(-0.705374, -0.668203),
            vec2( 0.977050, -0.108615),
            vec2( 0.063326,  0.142369),
            vec2( 0.203528,  0.214331),
            vec2(-0.667531,  0.326090),
            vec2(-0.098422, -0.295755),
            vec2(-0.885922,  0.215369),
            vec2( 0.566637,  0.605213),
            vec2( 0.039766, -0.396100)
        );

        float angle = fract(view.time * 0.27) * 6.2831853; // slowly rotates pattern
        mat2 rot = mat2(cos(angle), -sin(angle),
                        sin(angle),  cos(angle));

        vec3 accum = vec3(0.0);
        float invSamples = 1.0 / float(samples);

        for (int i = 0; i < samples; i++) {
            vec2 offset = rot * OFFSETS_16[i] * pixelFootprint * 0.8189;
            accum += water(uv + offset, vec3(1.0), view.time);
        }

        out_color = vec4(accum * invSamples, 1.0);
    }



    /*
        vec3 wave = vec3(0.0);
    #if ANTIALIAS
        float fac = 0.25;
        for(int y = 0; y < 2; y++) {
            for(int x = 0; x < 2; x++) {
                vec2 offset = vec2(0.5) * vec2(x, y) - vec2(0.25);
    #else
        float fac = 1.0;
        vec2 offset = vec2(0.0);
    #endif
                wave += water(uv + pixelSize * offset, vec3(1.0), view.time);
    #if ANTIALIAS
            }
        }
    #endif
        out_color = vec4(fac * wave, 1.0);*/
    } else {
        vec3 normal = normalize(frag_normal);

        Isometry modelview = push_constants.modelview;

        vec4 modelview_direction = vec4(modelview.rx, -modelview.ry, -modelview.rz, -modelview.rw);
        vec3 sun = rotate(normalize(vec3(1.0, 1.0, 1.0)), vec4(view.rx, view.ry, view.rz, view.rw));
        vec3 view = rotor_to_vec(modelview_direction);

        vec4 tex = texture(tex_sampler, vec2(frag_tex_coord.x, frag_tex_coord.y));

        if (tex.w < push_constants.material_properties.alpha_cutoff) discard;

        out_color = vec4(push_constants.alpha * tex.xyz * (
                        1.5 * lighting(vec3(0.0,0.0,-1.0), sun, sun_color, normal) +
                        0.1 * fill_light_color +
                        0.25 * lighting(view, vec3(-sun.x,sun.y,-sun.z), fill_light_color, normal)
                    ), push_constants.alpha * tex.w);
    }
}
