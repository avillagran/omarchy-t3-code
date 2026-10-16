#version 440
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float progress;
    float textWidth;
    float bandWidth;
} ubuf;
layout(binding = 1) uniform sampler2D source;
void main() {
    float x = (qt_TexCoord0.x * ubuf.textWidth + ubuf.bandWidth
             - ubuf.progress * (ubuf.textWidth + 2.0 * ubuf.bandWidth)) / ubuf.bandWidth;
    float d = abs(x - 0.5);
    float mask = d < 0.15 ? mix(1.0, 0.55, d / 0.15)
               : d < 0.35 ? mix(0.55, 0.12, (d - 0.15) / 0.20)
               : mix(0.12, 0.0, clamp((d - 0.35) / 0.15, 0.0, 1.0));
    fragColor = texture(source, qt_TexCoord0) * mask * ubuf.qt_Opacity;
}
