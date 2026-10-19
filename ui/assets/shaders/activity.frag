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
    // Gaussian falloff: the same peak and half-brightness width as the CSS
    // stops, without their visible kinks.
    float mask = exp(-(d * d) / 0.04);
    fragColor = texture(source, qt_TexCoord0) * mask * ubuf.qt_Opacity;
}
