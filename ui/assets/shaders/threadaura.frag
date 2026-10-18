#version 440
// Thread row aura, star themed:
//  working   - a comet orbits the card border with faint twinkling dust inside
//  attention - the border breathes and four-point glints sparkle on it
//  burst     - one-shot shooting star across the row when its state changes
// Styles (Preferences > Animations): working 0 comet / 1 pulse; attention
// 0 glints / 1 pulse / 2 static; burst 0 shooting star / 1 border flash.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float clock;
    float working;
    float attention;
    float burst;
    float radius;
    float seed;
    float intensity;
    float workingStyle;
    float attentionStyle;
    float burstStyle;
    vec2 size;
    vec4 tone;
    vec4 alert;
    vec4 flash;
} ubuf;

const float PI = 3.14159265;

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Signed distance to the rounded card outline (negative inside).
float box(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

// Arc length of the outline point nearest p, clockwise from the top-left.
float perimeter(vec2 p, vec2 b, float r) {
    float top = 2.0 * b.x, side = 2.0 * b.y, arc = 0.5 * PI * r;
    if (p.y < -b.y && abs(p.x) <= b.x) return p.x + b.x;
    if (p.x > b.x && p.y < -b.y) return top + (atan(p.y + b.y, p.x - b.x) + 0.5 * PI) * r;
    if (p.x > b.x && abs(p.y) <= b.y) return top + arc + p.y + b.y;
    if (p.x > b.x && p.y > b.y) return top + arc + side + atan(p.y - b.y, p.x - b.x) * r;
    if (p.y > b.y && abs(p.x) <= b.x) return top + 2.0 * arc + side + b.x - p.x;
    if (p.x < -b.x && p.y > b.y) return 2.0 * top + 2.0 * arc + side + (atan(p.y - b.y, p.x + b.x) - 0.5 * PI) * r;
    if (p.x < -b.x && abs(p.y) <= b.y) return 2.0 * top + 3.0 * arc + side + b.y - p.y;
    if (p.x < -b.x && p.y < -b.y) return 2.0 * top + 3.0 * arc + 2.0 * side + (atan(p.y + b.y, p.x + b.x) + PI) * r;
    // Deep interior: far from the outline, any value works.
    return 0.0;
}

// Four-point star glint.
float glint(vec2 d, float s) {
    vec2 a = abs(d);
    float rays = exp(-a.x * 6.0 / s) * exp(-a.y * 0.9 / s) + exp(-a.y * 6.0 / s) * exp(-a.x * 0.9 / s);
    return rays + exp(-dot(d, d) / (s * s * 0.5));
}

// Sparse twinkling dust; `live` fades it in and out.
float dust(vec2 q, float cell, float density, float rate) {
    vec2 id = floor(q / cell);
    float h = hash12(id + ubuf.seed);
    if (h > density) return 0.0;
    vec2 pos = (id + 0.2 + 0.6 * vec2(hash12(id + 3.1), hash12(id + 7.7))) * cell;
    float tw = max(0.0, sin(ubuf.clock * rate * (0.6 + h * 2.0) + h * 40.0));
    return exp(-dot(q - pos, q - pos) / 1.2) * tw * tw;
}

void main() {
    vec2 q = qt_TexCoord0 * ubuf.size;
    vec2 p = q - ubuf.size * 0.5;
    float r = ubuf.radius;
    vec2 b = max(ubuf.size * 0.5 - vec2(r) - 0.75, vec2(0.0));
    float d = box(p, b, r);
    float line = smoothstep(1.6, 0.0, abs(d));
    float inner = d < 0.0 ? exp(d / 5.0) : 0.0;

    vec3 rgb = vec3(0.0);
    float a = 0.0;

    if (ubuf.working > 0.001 && ubuf.workingStyle > 0.5) {
        float pulse = 0.5 + 0.5 * sin(ubuf.clock * 1.8 + ubuf.seed * 6.2832);
        float k = (line * (0.22 + 0.45 * pulse) + inner * (0.05 + 0.14 * pulse) + dust(q, 13.0, 0.16, 1.4) * 0.4) * ubuf.working;
        rgb += ubuf.tone.rgb * k;
        a += k;
    } else if (ubuf.working > 0.001) {
        float s = perimeter(p, b, r);
        float len = 4.0 * (b.x + b.y) + 2.0 * PI * r;
        float head = fract(ubuf.clock * (0.13 + 0.06 * ubuf.seed) + ubuf.seed) * len;
        float behind = mod(head - s + len, len);
        float tail = behind < len * 0.32 ? pow(1.0 - behind / (len * 0.32), 2.2) : 0.0;
        float nose = exp(-behind * behind / 18.0) + exp(-(len - behind) * (len - behind) / 4.0);
        float comet = (line * (tail * 0.75 + nose) + inner * (tail * 0.25 + nose * 0.6));
        float sparkle = dust(q, 13.0, 0.16, 1.4) * 0.45;
        float k = (comet + sparkle) * ubuf.working;
        vec3 c = mix(ubuf.tone.rgb, vec3(1.0), clamp(nose * 0.6, 0.0, 0.6));
        rgb += c * k;
        a += k;
    }

    if (ubuf.attention > 0.001) {
        float pulse = ubuf.attentionStyle > 1.5 ? 0.6 : 0.5 + 0.5 * sin(ubuf.clock * 2.6);
        float edge = line * (0.35 + 0.5 * pulse) + inner * (0.10 + 0.22 * pulse);
        // Glints take turns on the corners.
        float g = 0.0;
        for (int i = 0; i < 3 && ubuf.attentionStyle < 0.5; i++) {
            float fi = float(i);
            float phase = fract(ubuf.clock * 0.45 + fi / 3.0);
            float on = sin(phase * PI);
            vec2 at = fi < 0.5 ? vec2(b.x + r * 0.3, -b.y - r * 0.3)
                    : fi < 1.5 ? vec2(-b.x - r * 0.3, b.y + r * 0.3)
                    : vec2(b.x * 0.15, -b.y - r);
            g += glint(p - at, 1.4 + 1.4 * on) * on * on;
        }
        float k = (edge + g * 0.8) * ubuf.attention;
        rgb += mix(ubuf.alert.rgb, vec3(1.0), clamp(g * 0.5, 0.0, 0.5)) * k;
        a += k;
    }

    if (ubuf.burst > 0.0 && ubuf.burst < 1.0 && ubuf.burstStyle > 0.5) {
        float k = ubuf.burst;
        float m = (line * 0.9 + inner * 0.45) * pow(1.0 - k, 1.6) * smoothstep(0.0, 0.06, k);
        rgb += ubuf.flash.rgb * m;
        a += m;
    } else if (ubuf.burst > 0.0 && ubuf.burst < 1.0) {
        float k = ubuf.burst;
        float e = 1.0 - pow(1.0 - k, 2.4);
        vec2 dir = normalize(vec2(1.0, -0.22));
        vec2 head = vec2(mix(-0.15, 1.15, e) * ubuf.size.x, mix(0.78, 0.30, e) * ubuf.size.y);
        vec2 rel = q - head;
        float along = dot(rel, -dir);
        float perp = abs(dot(rel, vec2(-dir.y, dir.x)));
        float tlen = ubuf.size.x * 0.45;
        float t = clamp(1.0 - along / tlen, 0.0, 1.0);
        float streak = along > -1.5 ? smoothstep(0.7 + 1.3 * t, 0.0, perp) * t * t : 0.0;
        float star = glint(rel, 2.4);
        // Dust left behind the head, fading as the burst ends.
        float trail = q.x < head.x ? dust(q, 9.0, 0.35, 9.0) * smoothstep(1.0, 0.4, k) : 0.0;
        float ring = line * pow(1.0 - k, 2.0) * 0.7;
        float fade = smoothstep(0.0, 0.08, k) * smoothstep(1.0, 0.7, k);
        float m = (streak + star * 0.9) * fade + trail * 0.9 + ring;
        rgb += mix(ubuf.flash.rgb, vec3(1.0), clamp(star * fade, 0.0, 0.7)) * m;
        a += m;
    }

    rgb *= ubuf.intensity;
    a = clamp(a * ubuf.intensity, 0.0, 1.0);
    // Premultiplied: colour never exceeds coverage.
    fragColor = vec4(min(rgb, vec3(a)), a) * ubuf.qt_Opacity;
}
