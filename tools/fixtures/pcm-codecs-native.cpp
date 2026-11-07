// Build the synthetic conversion corpus with the complete original traits in
// Clang. The actual device adapter is compiled separately with the usual C++
// compiler; no copied conversion formula supplies its expected float values.
#include "../../native/voice/vendor/chromium/media/base/audio_sample_types.h"
#include <bit>
#include <filesystem>
#include <fstream>
#include <stdexcept>

int main(int argc,char **argv) {
    if(argc!=2||!std::filesystem::exists("/run/.containerenv"))return 2;
    if(std::endian::native!=std::endian::little)return 3;
    if(std::filesystem::exists(argv[1]))return 6;
    std::ofstream output(argv[1],std::ios::binary|std::ios::out);
    if(!output)return 4;
    output.write("O3CODEC1",8);
    constexpr uint32_t Count=131073;
    uint32_t random=0x13579bdf;
    constexpr uint32_t Edges[]={0,1,0x7fffffff,0x80000000,0x80000001,0x00ffffff,0x01000000,0x01000001,0x01000003,0x7fffff00,0x7fffff80,0xffffffff};
    for(unsigned kind=1;kind<=3;kind++)for(uint32_t index=0;index<Count;index++) {
        uint32_t bits=0;float value=0;
        if(kind==1) {
            auto sample=uint8_t(index);bits=sample;
            value=media::UnsignedInt8SampleTypeTraits::ToFloat(sample);
        } else if(kind==2) {
            auto sample=std::bit_cast<int16_t>(uint16_t(index));bits=uint16_t(index);
            value=media::SignedInt16SampleTypeTraits::ToFloat(sample);
        } else {
            random=random*1664525u+1013904223u;
            bits=index<std::size(Edges)?Edges[index]:random;
            value=media::SignedInt32SampleTypeTraits::ToFloat(std::bit_cast<int32_t>(bits));
        }
        output.write(reinterpret_cast<const char *>(&bits),4);
        output.write(reinterpret_cast<const char *>(&value),4);
    }
    return output.good()?0:5;
}
