#pragma once
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>

inline void write_u32(FILE* f, uint32_t v) {
    uint8_t buf[4];
    buf[0] = (uint8_t)(v);
    buf[1] = (uint8_t)(v >> 8);
    buf[2] = (uint8_t)(v >> 16);
    buf[3] = (uint8_t)(v >> 24);
    fwrite(buf, 1, 4, f);
}

inline void write_f64(FILE* f, double v) {
    // Assumes little-endian host (x86/ARM).
    uint8_t buf[8];
    memcpy(buf, &v, 8);
    fwrite(buf, 1, 8, f);
}

inline void write_bytes(FILE* f, const uint8_t* data, size_t len) {
    fwrite(data, 1, len, f);
}

inline void write_u8(FILE* f, uint8_t v) {
    fwrite(&v, 1, 1, f);
}

inline FILE* open_golden(const std::string& subdir, const std::string& name,
                         const std::string& ext) {
    std::string dir = "golden/" + subdir;
    std::string cmd = "mkdir -p " + dir;
    system(cmd.c_str());
    std::string path = dir + "/" + name + "." + ext;
    FILE* f = fopen(path.c_str(), "wb");
    if (!f) {
        fprintf(stderr, "Cannot open %s\n", path.c_str());
        exit(1);
    }
    return f;
}
