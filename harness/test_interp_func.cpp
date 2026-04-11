#include <cstdio>

extern "C" unsigned char rust_sample_adaptive_lum(
    const unsigned char* img_data, int img_w, int img_h,
    int ix, int iy, unsigned ox, unsigned oy,
    int sec_ox, int sec_oy, int sec_w, int sec_h
);

int main() {
    // Simple 4x4 image with known values
    unsigned char img[4*4] = {
         0,  0,  0,  0,
         0, 50,100,  0,
         0,100,200,  0,
         0,  0,  0,  0,
    };
    
    // Test various positions
    printf("Testing sample_adaptive_lum with 4x4 image:\n");
    for (unsigned oy = 0; oy <= 256; oy += 64) {
        for (unsigned ox = 0; ox <= 256; ox += 64) {
            unsigned char v = rust_sample_adaptive_lum(
                img, 4, 4,
                0, 0, ox, oy,
                0, 0, 4, 4
            );
            printf("  ix=0 iy=0 ox=%3u oy=%3u -> %d\n", ox, oy, v);
        }
    }
    
    // Compare: what should the values be for the center point?
    // The 4x4 kernel centered at (0,0) reads columns 0..3 and rows 0..3
    // with the center between cols 1-2 and rows 1-2.
    // At ox=128 (halfway between col 1 and 2): should interpolate 50 and 100 → ~75
    // At oy=128: should interpolate row 1 and 2 → ~75 for center column
    printf("\nCenter test (ox=128, oy=128):\n");
    unsigned char v = rust_sample_adaptive_lum(img, 4, 4, 0, 0, 128, 128, 0, 0, 4, 4);
    printf("  result=%d (expected ~75-100)\n", v);
    
    return 0;
}
