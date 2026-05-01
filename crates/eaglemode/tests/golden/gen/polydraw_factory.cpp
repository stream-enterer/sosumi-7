// Factory for emTestPanel::PolyDrawPanel — compiled separately against the scaffold
// header (using #define private public) to expose the private inner class.

#define private public
#include <emTest/emTestPanel.h>
#undef private

emPanel* create_polydraw(emPanel::ParentArg parent, const emString& name) {
    return new emTestPanel::PolyDrawPanel(parent, name);
}
