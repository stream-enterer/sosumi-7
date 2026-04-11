// Minimal test: single pixel blend comparison
#include <cstdio>
#include <cstring>

extern "C" void rust_blend_source_over_simple(
    unsigned char* dest, const unsigned char* src, int count, int opacity
);

int main() {
    // Match the divergent case: interp rgba(1,1,1,1), opacity=2373, canvas=transparent
    unsigned char interp[4] = {1, 1, 1, 1};
    unsigned char rust_dest[4] = {0, 0, 0, 0};
    
    rust_blend_source_over_simple(rust_dest, interp, 1, 2373);
    
    printf("Input: rgba(%d,%d,%d,%d) opacity=%d\n", interp[0], interp[1], interp[2], interp[3], 2373);
    printf("Rust output: rgba(%d,%d,%d,%d)\n", rust_dest[0], rust_dest[1], rust_dest[2], rust_dest[3]);
    
    // Also test with opacity=0x1000 (full)
    unsigned char rust_dest2[4] = {0, 0, 0, 0};
    rust_blend_source_over_simple(rust_dest2, interp, 1, 0x1000);
    printf("Rust output (full opacity): rgba(%d,%d,%d,%d)\n", rust_dest2[0], rust_dest2[1], rust_dest2[2], rust_dest2[3]);
    
    // Test with higher values
    unsigned char interp3[4] = {128, 64, 32, 200};
    unsigned char rust_dest3[4] = {0, 0, 0, 0};
    rust_blend_source_over_simple(rust_dest3, interp3, 1, 0x1000);
    printf("Rust output (128,64,32,200 full): rgba(%d,%d,%d,%d)\n", 
           rust_dest3[0], rust_dest3[1], rust_dest3[2], rust_dest3[3]);
    
    return 0;
}
