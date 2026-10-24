#version 440
// Procedural starfield: three parallax layers drifting with `drift`, twinkle
// and shooting stars on `clock`. Stars use panel coordinates (origin + local)
// so stacked instances line up; meteors stay local to each instance.
layout(location = 0) in vec2 qt_TexCoord0;
layout(location = 0) out vec4 fragColor;
layout(std140, binding = 0) uniform buf {
    mat4 qt_Matrix;
    float qt_Opacity;
    float drift;
    float clock;
    float meteors;
    float brightness;
    float nebula;
    float scroll;
    float fade;
    vec2 size;
    vec2 origin;
    vec4 star;
    vec4 accent;
    vec4 base;
    vec4 meteor;
    float ink;
} ubuf;

float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// One star per occupied cell, kept off the cell edges so it never clips.
float layer(vec2 p, float cell, float density, float seed, float radius, out float tint) {
    vec2 id = floor(p / cell);
    vec2 f = fract(p / cell);
    float h = hash12(id + seed);
    tint = hash12(id + seed + 5.3);
    if (h > density) return 0.0;
    vec2 pos = 0.2 + 0.6 * hash22(id + seed * 1.7);
    float d = length((f - pos) * cell);
    float r = radius * mix(0.55, 1.25, hash12(id + seed + 3.1));
    float rate = 0.6 + 2.2 * hash12(id + seed + 7.0);
    float twinkle = 0.55 + 0.45 * sin(ubuf.clock * rate + h * 61.0);
    float core = smoothstep(r + 0.9, r * 0.2, d);
    float halo = exp(-d * d / (r * r * 7.0)) * 0.3;
    return (core + halo) * twinkle * mix(0.45, 1.0, hash12(id + seed + 11.0));
}

// Shooting stars: each channel rolls once per period whether it fires.
float meteor(vec2 q, float channel) {
    float period = 2.1 + channel * 1.37;
    float slot = floor(ubuf.clock / period + channel * 0.31);
    float t = fract(ubuf.clock / period + channel * 0.31) * period;
    vec2 key = vec2(slot, channel * 17.0);
    if (hash12(key) > ubuf.meteors) return 0.0;
    float duration = 0.75 + 0.5 * hash12(key + 2.0);
    if (t > duration) return 0.0;
    float k = t / duration;
    vec2 dir = normalize(vec2(-1.0, 0.28 + 0.35 * hash12(key + 4.0)));
    float span = length(ubuf.size);
    vec2 start = vec2(ubuf.size.x * (0.35 + 0.75 * hash12(key + 6.0)),
                      ubuf.size.y * (-0.15 + 0.6 * hash12(key + 8.0)));
    vec2 head = start + dir * span * 0.9 * k;
    float len = clamp(span * 0.22, 60.0, 240.0);
    vec2 rel = q - head;
    float along = dot(rel, -dir);
    float perp = abs(dot(rel, vec2(-dir.y, dir.x)));
    float fade = smoothstep(0.0, 0.12, k) * smoothstep(1.0, 0.55, k);
    float tail = clamp(1.0 - along / len, 0.0, 1.0);
    float streak = along > -1.5 ? smoothstep(0.6 + 1.1 * tail, 0.0, perp) * tail * tail : 0.0;
    float glow = exp(-dot(rel, rel) / 10.0);
    return (streak + glow) * fade;
}

void main() {
    vec2 q = qt_TexCoord0 * ubuf.size;
    vec2 p = q + ubuf.origin;
    vec2 flow = vec2(ubuf.drift, ubuf.drift * 0.18);
    // Scroll parallax: nearer layers follow the content further.
    vec2 lift = vec2(0.0, ubuf.scroll);
    float t1, t2, t3;
    float s = layer(p + flow * 0.30 + lift * 0.06, 29.0, 0.42, 1.0, 0.55, t1) * 0.55
            + layer(p + flow * 0.62 + lift * 0.12, 53.0, 0.38, 19.0, 0.8, t2) * 0.8
            + layer(p + flow * 1.00 + lift * 0.22, 97.0, 0.32, 43.0, 1.15, t3);
    float tint = (t1 + t2 + t3) / 3.0;
    vec3 starColor = mix(ubuf.star.rgb, ubuf.accent.rgb, smoothstep(0.55, 1.0, tint) * 0.6);
    float m = 0.0;
    if (ubuf.meteors > 0.001)
        m = meteor(q, 0.0) + meteor(q, 1.0) + meteor(q, 2.0) + meteor(q, 3.0);
    // Optional soft bottom edge: the instance dissolves into the sky below.
    float edge = ubuf.fade > 0.0 ? 1.0 - smoothstep(1.0 - ubuf.fade, 1.0, qt_TexCoord0.y) : 1.0;
    // ink > 1 on light skies: dark stars need more coverage to read on white.
    float light = clamp(s * ubuf.brightness * ubuf.ink, 0.0, 1.0) * edge;
    float shoot = clamp(m * ubuf.ink, 0.0, 1.0) * edge;

    // Base sky with a faint accent nebula toward the leading edge.
    vec2 uv = qt_TexCoord0;
    float cloud = ubuf.nebula * (0.55 * (1.0 - uv.x) + 0.25 * (1.0 - uv.y))
                * (0.8 + 0.2 * sin(ubuf.clock * 0.25 + uv.x * 3.0));
    vec3 sky = mix(ubuf.base.rgb, ubuf.accent.rgb, clamp(cloud, 0.0, 1.0) * 0.35);
    float skyA = ubuf.base.a * edge;

    vec3 rgb = sky * skyA;
    float a = skyA;
    rgb = rgb * (1.0 - light) + starColor * light;
    a = a + light * (1.0 - a);
    vec3 meteorColor = mix(ubuf.meteor.rgb, ubuf.accent.rgb, 0.25);
    rgb = rgb * (1.0 - shoot) + meteorColor * shoot;
    a = a + shoot * (1.0 - a);
    fragColor = vec4(rgb, a) * ubuf.qt_Opacity;
}
