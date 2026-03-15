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
    std::string dir = "tests/golden/data/" + subdir;
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

/// Write a length-prefixed string: [u32 len][len bytes].
inline void write_string(FILE* f, const char* s) {
    uint32_t len = (uint32_t)strlen(s);
    write_u32(f, len);
    if (len > 0) fwrite(s, 1, len, f);
}

/// Dump a trajectory: [u32 step_count][step_count * (f64 rel_x, f64 rel_y, f64 rel_a)]
inline void dump_trajectory(const char* name, const double* data, uint32_t steps) {
    FILE* f = open_golden("trajectory", name, "trajectory.golden");
    write_u32(f, steps);
    for (uint32_t i = 0; i < steps; i++) {
        write_f64(f, data[i * 3 + 0]); // rel_x
        write_f64(f, data[i * 3 + 1]); // rel_y
        write_f64(f, data[i * 3 + 2]); // rel_a
    }
    fclose(f);
    printf("  trajectory/%s\n", name);
}
