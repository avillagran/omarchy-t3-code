#pragma once
// These functions are linked privately into the Qt voice module.
#if defined(__GNUC__) || defined(__clang__)
#define V8_BASE_EXPORT __attribute__((visibility("hidden")))
#else
#define V8_BASE_EXPORT
#endif
