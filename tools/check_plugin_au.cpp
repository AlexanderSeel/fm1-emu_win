// Local AU bundle smoke test without installing plugins or changing the AU registry.
// clang++ -std=c++17 tools/check_plugin_au.cpp -framework AudioToolbox -framework CoreFoundation -o /tmp/fm1-check-au
// /tmp/fm1-check-au /absolute/path/to/FM-1.component
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <cmath>
#include <cstring>
#include <iostream>
#include <stdexcept>

static void check(OSStatus status, const char* action) {
    if (status != noErr) throw std::runtime_error(std::string(action) + ": " + std::to_string(status));
}
int main(int argc, char** argv) {
    if (argc != 2) { std::cerr << "usage: check_plugin_au /path/to/FM-1.component\n"; return 2; }
    try {
        auto url = CFURLCreateFromFileSystemRepresentation(nullptr, reinterpret_cast<const UInt8*>(argv[1]), std::strlen(argv[1]), true);
        auto bundle = CFBundleCreate(nullptr, url);
        CFRelease(url);
        if (!bundle || !CFBundleLoadExecutable(bundle)) throw std::runtime_error("could not load AU bundle");
        auto info = CFBundleGetInfoDictionary(bundle);
        auto components = static_cast<CFArrayRef>(CFDictionaryGetValue(info, CFSTR("AudioComponents")));
        auto component = static_cast<CFDictionaryRef>(CFArrayGetValueAtIndex(components, 0));
        auto factoryName = static_cast<CFStringRef>(CFDictionaryGetValue(component, CFSTR("factoryFunction")));
        auto factory = reinterpret_cast<AudioComponentFactoryFunction>(CFBundleGetFunctionPointerForName(bundle, factoryName));
        if (!factory) throw std::runtime_error("AU factory function is missing");
        AudioComponentDescription description { kAudioUnitType_MusicDevice, 'Fm1s', 'Fm1e', 0, 0 };
        // AudioComponentRegister registers only in this process, unlike installation/auval.
        auto registration = AudioComponentRegister(&description, CFSTR("fm1-emu: local save test"), 256, factory);
        if (!registration) throw std::runtime_error("could not register local AU factory");
        AudioUnit unit = nullptr;
        check(AudioComponentInstanceNew(registration, &unit), "instantiate");
        AudioStreamBasicDescription format {};
        format.mSampleRate = 44100;
        format.mFormatID = kAudioFormatLinearPCM;
        format.mFormatFlags = kAudioFormatFlagsNativeFloatPacked | kAudioFormatFlagIsNonInterleaved;
        format.mBytesPerPacket = format.mBytesPerFrame = sizeof(float);
        format.mFramesPerPacket = 1;
        format.mChannelsPerFrame = 2;
        format.mBitsPerChannel = 32;
        check(AudioUnitSetProperty(unit, kAudioUnitProperty_StreamFormat, kAudioUnitScope_Output, 0, &format, sizeof(format)), "stereo format");
        check(AudioUnitInitialize(unit), "initialize");
        float left[128] {}, right[128] {};
        struct { UInt32 count; AudioBuffer buffers[2]; } audio { 2, {{1, sizeof(left), left}, {1, sizeof(right), right}} };
        AudioTimeStamp time {}; time.mFlags = kAudioTimeStampSampleTimeValid;
        for (int i = 0; i < 32; ++i) {
            AudioUnitRenderActionFlags flags = 0;
            check(AudioUnitRender(unit, &flags, &time, 0, 128, reinterpret_cast<AudioBufferList*>(&audio)), "render");
            for (int frame = 0; frame < 128; ++frame) {
                if (!std::isfinite(left[frame]) || !std::isfinite(right[frame])) throw std::runtime_error("non-finite audio");
            }
            time.mSampleTime += 128;
        }
        CFPropertyListRef state = nullptr;
        UInt32 size = sizeof(state);
        check(AudioUnitGetProperty(unit, kAudioUnitProperty_ClassInfo, kAudioUnitScope_Global, 0, &state, &size), "get project state");
        check(AudioUnitSetProperty(unit, kAudioUnitProperty_ClassInfo, kAudioUnitScope_Global, 0, &state, sizeof(state)), "restore project state");
        if (state) CFRelease(state);
        check(AudioUnitUninitialize(unit), "uninitialize");
        check(AudioComponentInstanceDispose(unit), "dispose");
        CFRelease(bundle);
        std::cout << "PASS: local AU bundle loads, initializes, renders stereo audio and round-trips project state.\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "FAIL: " << error.what() << '\n'; return 1;
    }
}
