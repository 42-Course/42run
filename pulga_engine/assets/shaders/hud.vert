#version 410 core

// Pixels, origin at the window's top-left corner, y pointing down.
layout(location = 0) in vec2 inPos;
layout(location = 1) in vec2 inUV;
layout(location = 2) in vec4 inColor;

out vec2 vUV;
out vec4 vColor;

uniform vec2 screen; // window size in pixels

void main() {
    vUV = inUV;
    vColor = inColor;
    vec2 ndc = vec2(inPos.x / screen.x * 2.0 - 1.0, 1.0 - inPos.y / screen.y * 2.0);
    gl_Position = vec4(ndc, 0.0, 1.0);
}
