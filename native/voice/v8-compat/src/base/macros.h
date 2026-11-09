#pragma once
#include <bit>
#include <cstdint>
#include <limits>
#include <type_traits>

#define V8_INLINE inline
#if defined(__GNUC__) || defined(__clang__)
#define V8_WARN_UNUSED_RESULT __attribute__((warn_unused_result))
#else
#define V8_WARN_UNUSED_RESULT
#endif
namespace v8::base {
using std::bit_cast;
}
