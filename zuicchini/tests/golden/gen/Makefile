EM_ROOT := $(HOME)/.local/git/eaglemode-0.96.4
CXX     := g++
CXXFLAGS := -O2 -std=c++17 -I$(EM_ROOT)/include
LDFLAGS  := -L$(EM_ROOT)/lib -Wl,-rpath,$(EM_ROOT)/lib -lemCore -lemTestPanel -lm -lpthread -ldl

gen_golden: gen_golden.cpp golden_format.h
	$(CXX) $(CXXFLAGS) -o $@ $< $(LDFLAGS)

run: gen_golden
	cd ../../.. && EM_DIR=$(EM_ROOT) LD_LIBRARY_PATH=$(EM_ROOT)/lib:$$LD_LIBRARY_PATH tests/golden/gen/gen_golden

clean:
	rm -f gen_golden

.PHONY: run clean
