# vim: ts=4:sw=4:noexpandtab:
#
# Syntax highlighting for 703 assembly, vim and VS Code. The files are
# generated from asm703.py's tables and checked in; `make` regenerates them.
# The installs are symlinks back into this directory, so a regenerated
# grammar is live without installing again.

VIMDIR  ?= $(HOME)/.vim
# the local VS Code's extensions, and a Remote-SSH server's on this host
VSCODE_DIRS ?= $(HOME)/.vscode/extensions $(HOME)/.vscode-server/extensions
EXT_NAME := local.asm703

.PHONY: all check install install-vim install-vscode uninstall

all:
	python3 gen_syntax.py

check:
	python3 gen_syntax.py --check

install: install-vim install-vscode

install-vim:
	mkdir -p $(VIMDIR)/syntax $(VIMDIR)/ftdetect
	ln -sfn $(CURDIR)/vim/syntax/asm703.vim $(VIMDIR)/syntax/asm703.vim
	ln -sfn $(CURDIR)/vim/ftdetect/asm703.vim $(VIMDIR)/ftdetect/asm703.vim

# VS Code adopts a folder that appears in its extensions directory into its
# extensions.json -- a running one within a second, otherwise at startup --
# and Reload Window activates it.
install-vscode:
	@for d in $(VSCODE_DIRS); do \
		if [ -d $$d ]; then ln -sfn $(CURDIR)/vscode/asm703 $$d/$(EXT_NAME) && echo "linked $$d/$(EXT_NAME)"; fi; \
	done

uninstall:
	rm -f $(VIMDIR)/syntax/asm703.vim $(VIMDIR)/ftdetect/asm703.vim
	@for d in $(VSCODE_DIRS); do rm -f $$d/$(EXT_NAME); done
