#include "voicemath.h"
#include "vendor/v8/src/base/ieee754.h"

double O3VoiceMath::sin(double value) const { return v8::base::ieee754::sin(value); }
double O3VoiceMath::cos(double value) const { return v8::base::ieee754::cos(value); }
double O3VoiceMath::exp(double value) const { return v8::base::ieee754::exp(value); }

#include "voicemath.moc"
