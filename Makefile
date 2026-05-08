CARGO  ?= $(shell command -v cargo)
DEVDIR  = dev
BINARY  = cmdlog
BINDIR  = bin

.PHONY: build release test install clean

build:
	cd $(DEVDIR) && $(CARGO) build
	rm -f ./$(BINARY) && cp $(DEVDIR)/target/debug/$(BINARY) ./$(BINARY)

release:
	cd $(DEVDIR) && $(CARGO) build --release
	rm -f ./$(BINARY) && cp $(DEVDIR)/target/release/$(BINARY) ./$(BINARY)

test:
	bash $(DEVDIR)/run_tests.sh

install: release
	mkdir -p $(BINDIR)
	cp ./$(BINARY) $(BINDIR)/$(BINARY)

clean:
	cd $(DEVDIR) && $(CARGO) clean
