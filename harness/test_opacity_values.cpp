// Test: compare C++ opacity values (ax1, ay1, ax2, ay2) from PaintRect
// against Rust SubPixelEdges coverage for the same coordinates.
//
// C++ PaintRect computes:
//   ix = (int)(x * 0x1000)
//   ax1 = 0x1000 - (ix & 0xfff)
//   ax2 = ((int)(x2 * 0x1000) + 0xfff) & 0xfff + 1
//   ay1 = 0x1000 - ((int)(y * 0x1000) & 0xfff)
//   ay2 = (int)(y2 * 0x1000) & 0xfff
//
// Rust SubPixelEdges computes:
//   frac_left = 0x1000 - Fixed12(dx_px).frac()
//   frac_right = Fixed12(dx_px + dw_px).frac()
//   etc.
//
// If these differ for the same pixel coordinates, the opacity applied to
// each interpolated pixel differs, causing the +5 divergence.

#include <cstdio>
#include <cmath>

// Rust function to get coverage for a specific pixel
extern "C" int rust_get_coverage(
    double dx_px, double dy_px, double dw_px, double dh_px,
    int px, int py
);

// C++ PaintRect opacity computation (emPainter.cpp lines 358-395)
struct CppOpacity {
    int ix, iy, ix2, iy2;
    int ax1, ax2, ay1, ay2;
    int iw;

    void compute(double x, double y, double w, double h) {
        double x2 = x + w;
        double y2 = y + h;

        ix = (int)(x * 0x1000);
        int ixe = ((int)(x2 * 0x1000)) + 0xfff;
        ax1 = 0x1000 - (ix & 0xfff);
        ax2 = (ixe & 0xfff) + 1;
        ix >>= 12;
        ixe >>= 12;
        iw = ixe - ix;
        if (iw <= 1 && iw > 0) {
            ax1 += ax2 - 0x1000;
        }

        iy = (int)(y * 0x1000);
        int iy2_raw = (int)(y2 * 0x1000);
        ay1 = 0x1000 - (iy & 0xfff);
        ay2 = iy2_raw & 0xfff;
        iy >>= 12;
        this->iy2 = iy2_raw >> 12;
        ix2 = ixe;
        if (iy >= this->iy2) {
            ay1 += ay2 - 0x1000;
            ay2 = 0;
        }
    }

    // Get the opacity for a specific pixel, matching PaintScanline's 3-value system
    int get_opacity(int px, int py) const {
        int o_y;
        if (py == iy && py < iy2) {
            o_y = ay1;
        } else if (py == iy2 && ay2 > 0) {
            o_y = ay2;
        } else if (py > iy && py < iy2) {
            o_y = 0x1000;
        } else if (py == iy && iy >= iy2) {
            o_y = ay1; // single-row case
        } else {
            return 0;
        }

        int o_x;
        if (px == ix && iw > 1) {
            o_x = ax1;
        } else if (px == ix + iw - 1 && iw > 1) {
            o_x = ax2;
        } else if (px == ix && iw <= 1) {
            o_x = ax1; // single-col case (ax1 already adjusted)
        } else {
            o_x = 0x1000;
        }

        // Combined: C++ does (ax * ay + 0x7ff) >> 12 for corner pixels,
        // but for edge pixels the opacity is just o_y (interior of X)
        // or o_x (interior of Y).
        if (o_x == 0x1000) return o_y;
        if (o_y == 0x1000) return o_x;
        return (o_x * o_y + 0x7ff) >> 12;
    }
};

int main() {
    // Test with checkbox-like coordinates
    // dest rect in pixel space: tex_x * ScaleX = 0.013026 * 800 = 10.4208
    // tex_w * ScaleX = 0.096974 * 800 = 77.5792
    double dx_px = 0.013026 * 800.0;
    double dy_px = 0.013026 * 800.0;
    double dw_px = 0.096974 * 800.0;
    double dh_px = 0.096974 * 800.0;

    CppOpacity cpp;
    cpp.compute(dx_px, dy_px, dw_px, dh_px);

    printf("C++ rect: ix=%d iy=%d ix2=%d iy2=%d iw=%d\n",
           cpp.ix, cpp.iy, cpp.ix2, cpp.iy2, cpp.iw);
    printf("C++ frac: ax1=%d ax2=%d ay1=%d ay2=%d\n",
           cpp.ax1, cpp.ax2, cpp.ay1, cpp.ay2);

    // Compare for pixels around the edges
    int mismatches = 0;
    int total = 0;

    // Only test pixels that PaintRect actually renders:
    // rows iy..iy2-1 for interior, iy2 only if ay2 > 0
    int y_end = cpp.ay2 > 0 ? cpp.iy2 : cpp.iy2 - 1;
    int x_end = cpp.ix + cpp.iw - 1;
    for (int py = cpp.iy; py <= y_end && py <= cpp.iy + 3; py++) {
        for (int px = cpp.ix; px <= x_end && px <= cpp.ix + 5; px++) {
            int cpp_cov = cpp.get_opacity(px, py);
            int rust_cov = rust_get_coverage(dx_px, dy_px, dw_px, dh_px, px, py);
            total++;
            if (cpp_cov != rust_cov) {
                printf("MISMATCH: px=%d py=%d cpp=%d rust=%d diff=%d\n",
                       px, py, cpp_cov, rust_cov, rust_cov - cpp_cov);
                mismatches++;
            }
        }
    }

    // Also test interior + right/bottom edges
    for (int py = y_end - 1; py <= y_end; py++) {
        for (int px = x_end - 1; px <= x_end; px++) {
            if (px < cpp.ix || py < cpp.iy) continue;
            int cpp_cov = cpp.get_opacity(px, py);
            int rust_cov = rust_get_coverage(dx_px, dy_px, dw_px, dh_px, px, py);
            total++;
            if (cpp_cov != rust_cov) {
                printf("MISMATCH: px=%d py=%d cpp=%d rust=%d diff=%d\n",
                       px, py, cpp_cov, rust_cov, rust_cov - cpp_cov);
                mismatches++;
            }
        }
    }

    // Test a few interior pixels
    int mid_x = (cpp.ix + cpp.ix2) / 2;
    int mid_y = (cpp.iy + cpp.iy2) / 2;
    for (int py = mid_y; py <= mid_y + 2; py++) {
        for (int px = mid_x; px <= mid_x + 2; px++) {
            int cpp_cov = cpp.get_opacity(px, py);
            int rust_cov = rust_get_coverage(dx_px, dy_px, dw_px, dh_px, px, py);
            total++;
            if (cpp_cov != rust_cov) {
                printf("MISMATCH: px=%d py=%d cpp=%d rust=%d diff=%d\n",
                       px, py, cpp_cov, rust_cov, rust_cov - cpp_cov);
                mismatches++;
            }
        }
    }

    if (mismatches == 0) {
        printf("PASS: All %d opacity/coverage values match.\n", total);
    } else {
        printf("FAIL: %d mismatches out of %d.\n", mismatches, total);
    }
    return mismatches > 0 ? 1 : 0;
}
