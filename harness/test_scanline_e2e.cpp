// End-to-end scanline comparison: run the ACTUAL C++ PaintScanline via
// libemCore.so and compare against the Rust golden test output.
//
// Strategy:
// 1. Run the C++ golden generator → get reference image
// 2. Run the Rust golden test → get actual image
// 3. Compare specific scanlines at the divergent coordinates
//
// Both images already exist as golden test artifacts:
// - C++ reference: crates/eaglemode/tests/golden/data/compositor/widget_checkbox_unchecked.compositor.golden
// - Rust actual: generated at test time
//
// Actually, we can just look at the DUMP_GOLDEN diff images which show
// exactly where the divergence is. But what we need is per-scanline
// intermediate state.
//
// Simplest approach that uses the harness: modify the C++ golden generator
// to dump raw scanline pixels for a specific row at the divergent location,
// then write a Rust test that renders the same row and compares.
//
// But that requires modifying gen_golden.cpp. Instead, let's compare the
// two full output images byte-by-byte at the divergent rows.

#include <cstdio>
#include <cstdlib>
#include <cstring>

// Read a binary golden file: u32 width, u32 height, then w*h*4 RGBA bytes
static unsigned char* read_golden(const char* path, int* w, int* h) {
    FILE* f = fopen(path, "rb");
    if (!f) { fprintf(stderr, "Cannot open %s\n", path); return NULL; }

    unsigned buf[2];
    if (fread(buf, 4, 2, f) != 2) { fclose(f); return NULL; }
    *w = buf[0];
    *h = buf[1];

    int size = (*w) * (*h) * 4;
    unsigned char* data = (unsigned char*)malloc(size);
    if (fread(data, 1, size, f) != (size_t)size) { free(data); fclose(f); return NULL; }
    fclose(f);
    return data;
}

int main(int argc, char** argv) {
    const char* cpp_path = "crates/eaglemode/tests/golden/data/compositor/"
                           "widget_checkbox_unchecked.compositor.golden";

    int cw, ch;
    unsigned char* cpp_img = read_golden(cpp_path, &cw, &ch);
    if (!cpp_img) {
        fprintf(stderr, "Cannot read C++ golden: %s\n", cpp_path);
        return 1;
    }
    printf("C++ golden: %dx%d\n", cw, ch);

    // Now we need the Rust output. We can get it by running the golden test
    // with DUMP_GOLDEN=1 which saves actual/expected PPM files. But the actual
    // raw data is what we need.
    //
    // The simplest check: compare specific rows in the C++ golden image against
    // themselves to understand the expected pixel values at the divergent location.
    // Then we know what C++ produces and can compare against Rust.

    // Divergent pixels are at x=24-35, y=288-305.
    // Let's dump row 290 (middle of divergent region) for x=20-40.
    int row = 290;
    if (row >= ch) { printf("Row %d out of range\n", row); free(cpp_img); return 1; }

    printf("\nC++ row %d, x=20..40:\n", row);
    for (int x = 20; x <= 40; x++) {
        int off = (row * cw + x) * 4;
        printf("  x=%d: rgb(%d,%d,%d) a=%d\n", x,
               cpp_img[off], cpp_img[off+1], cpp_img[off+2], cpp_img[off+3]);
    }

    // Now we need Rust's output for the same pixels. We can't get it from this
    // C++ program. But we CAN write the C++ expected values to a file that a
    // Rust test can read and compare against.

    const char* dump_path = "/tmp/cpp_scanline_290.bin";
    FILE* f = fopen(dump_path, "wb");
    if (f) {
        // Write x range, then RGBA bytes
        int x_start = 20, x_end = 40;
        fwrite(&x_start, 4, 1, f);
        fwrite(&x_end, 4, 1, f);
        fwrite(&row, 4, 1, f);
        for (int x = x_start; x <= x_end; x++) {
            int off = (row * cw + x) * 4;
            fwrite(&cpp_img[off], 1, 4, f);
        }
        fclose(f);
        printf("\nDumped C++ row %d x=[%d,%d] to %s\n", row, x_start, x_end, dump_path);
    }

    free(cpp_img);
    return 0;
}
