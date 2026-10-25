#version 410 core

in vec3 vWorldNormal;

out vec4 outColor;

uniform vec4 color;

// One fixed directional light plus ambient, so faces of a flat-colored
// mesh still read as distinct.
void main() {
    vec3 n = normalize(vWorldNormal);
    float ndl = clamp(dot(n, normalize(vec3(0.4, 0.7, 0.6))), 0.0, 1.0);
    float shade = 0.4 + 0.6 * ndl;
    outColor = vec4(color.rgb * shade, color.a);
}
