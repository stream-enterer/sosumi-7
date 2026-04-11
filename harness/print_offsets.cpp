// Print the byte offset of CanCpuDoAvx2 within SharedModel,
// and CoreConfig, so we can place them in the fake model buffer.
#include <cstdio>
#include <cstddef>
#include <emCore/emPainter.h>

int main() {
    printf("sizeof(SharedModel) is not available (private constructor)\n");
    printf("But we can compute offsets from the class layout:\n");

    // SharedModel fields (from emPainter.h):
    //   emRef<emCoreConfig> CoreConfig;
    //   emRef<emFontCache> FontCache;
    //   emPainter::SharedPixelFormat * PixelFormatList;
    //   bool CanCpuDoAvx2;  (only if EM_HAVE_X86_INTRINSICS)

    // We can't use offsetof on SharedModel (private), but we can
    // look at the disassembly to see what offset Init reads from Model.
    // Or we can just try different offsets.
    //
    // SharedModel inherits from emModel.
    // emModel inherits from emEngine.
    // emEngine inherits from emUncopyable.
    //
    // Let's print sizes of the base classes:
    printf("sizeof(void*) = %zu\n", sizeof(void*));

    // The simplest approach: just make the fake buffer large and
    // zero-filled. CanCpuDoAvx2=false (0) is what we want.
    // But CoreConfig is an emRef<emCoreConfig> — a pointer.
    // If Model->CanCpuDoAvx2 is false, line 103-108 takes the else path
    // and never reads CoreConfig. Same for line 254-260.
    //
    // The problem might be that CanCpuDoAvx2 is NOT at offset 0
    // within the fake buffer. It's after CoreConfig, FontCache,
    // PixelFormatList, plus all the base class data.

    printf("The issue: CanCpuDoAvx2 is at a large offset within SharedModel.\n");
    printf("Zero-filled buffer means CanCpuDoAvx2=0 (false) ONLY if\n");
    printf("it happens to be at a zeroed position. Since the ENTIRE buffer\n");
    printf("is zeroed, this should work... unless the read is at an\n");
    printf("offset beyond the buffer.\n");
    return 0;
}
