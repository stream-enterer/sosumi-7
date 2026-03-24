EM_ROOT := $(HOME)/git/eaglemode-0.96.4
CXX     := g++
CXXFLAGS := -O2 -std=c++17 -I$(EM_ROOT)/include -I$(EM_ROOT)
LDFLAGS  := -L$(EM_ROOT)/lib -Wl,-rpath,$(EM_ROOT)/lib -lemCore -lemTestPanel -lm -lpthread -ldl

OUTDIR  := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/../../../target/golden-gen)

$(OUTDIR)/gen_golden: gen_golden.cpp $(OUTDIR)/tktest_factory.o golden_format.h
	@mkdir -p $(OUTDIR)
	$(CXX) $(CXXFLAGS) -o $@ gen_golden.cpp $(OUTDIR)/tktest_factory.o $(LDFLAGS)

$(OUTDIR)/tktest_factory.o: tktest_factory.cpp
	@mkdir -p $(OUTDIR)
	$(CXX) $(CXXFLAGS) -c -o $@ $<

run: $(OUTDIR)/gen_golden
	cd ../../.. && EM_DIR=$(EM_ROOT) EM_USER_CONFIG_DIR=$${TMPDIR:-/tmp}/em_golden_config LD_LIBRARY_PATH=$(EM_ROOT)/lib:$$LD_LIBRARY_PATH $(OUTDIR)/gen_golden

clean:
	rm -f $(OUTDIR)/gen_golden $(OUTDIR)/tktest_factory.o

.PHONY: run clean
