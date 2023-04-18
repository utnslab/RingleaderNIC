# Copyright (c) Microsoft Corporation.
# Licensed under the MIT license.
export PREFIX ?= $(HOME)

export CONFIG_PATH ?= $(HOME)/config.yaml

export CARGO ?= $(HOME)/.cargo/bin/cargo
export TIMEOUT ?= 30

export SRCDIR = $(CURDIR)/src
export BINDIR = $(CURDIR)/bin
export LIBDIR = $(CURDIR)/lib
export CONTRIBDIR = $(CURDIR)/submodules
export BUILDDIR = $(CURDIR)/build

#===============================================================================
export BUILD ?= --release

export CARGO_FLAGS ?= $(BUILD) 

#===============================================================================

all: demikernel-examples

clean: demikernel-clean

demikernel-examples:
	cd $(SRCDIR) && \
        $(CARGO) build --examples $(CARGO_FLAGS)

demikernel-clean:
	cd $(SRCDIR) &&   \
	$(CARGO) clean \
