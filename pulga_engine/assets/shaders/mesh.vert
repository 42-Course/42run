#version 410 core

layout(location = 0) in vec3 inPos;
layout(location = 1) in vec3 inNormal;

out vec3 vWorldNormal;

uniform mat4 proj;
uniform mat4 view;
uniform mat4 model;

void main() {
    vWorldNormal = mat3(model) * inNormal;
    gl_Position = proj * view * model * vec4(inPos, 1.0);
}
