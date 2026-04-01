// Scaffold header: make emTestPanel's private TkTest accessible to the factory.
// Uses a #define hack to expose the private class.
#define private public
#include <emTest/emTestPanel.h>
#undef private
