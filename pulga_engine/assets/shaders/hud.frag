#version 410 core

in vec2 vUV;
in vec4 vColor;

out vec4 outColor;

uniform sampler2D atlas; // 1-channel font atlas: 255 = glyph pixel

void main() {
    if (texture(atlas, vUV).r < 0.5) discard;
    outColor = vColor;
}
