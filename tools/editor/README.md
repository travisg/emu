# Editor support for 703 assembly

Syntax highlighting for the `tools/asm703.py` dialect, for vim and VS Code.

```bash
make -C tools/editor install        # both; or install-vim / install-vscode
make -C tools/editor uninstall
```

Each install symlinks back into this directory: `~/.vim/syntax/asm703.vim` and
`~/.vim/ftdetect/asm703.vim` for vim, and `local.asm703` in `~/.vscode/extensions`
(and in `~/.vscode-server/extensions` if that exists, for Remote-SSH) for VS Code.
Use Reload Window in VS Code after the first install.

A file counts as 703 source by where it lives (`rex/**/*.asm`, `test/703/**/*.asm`), not by
its `.asm` extension, which every assembler uses. VS Code's `filenamePatterns` and vim's
ftdetect both take precedence over a generic `.asm` mapping. To mark a file somewhere else,
put `ft=asm703` in its modeline (vim), or choose *Raytheon 703 assembly* from the
language picker (VS Code).

## What is highlighted

The label in column 1, which may be local (`.LOOP`), then the op field, then the operand
and comment. The op field is coloured by class:

| class | mnemonics | why it has its own colour |
|---|---|---|
| skip | the 16 skips, `IXS DXS SXP SXM` | each acts on the following word |
| page selection | `SML SMU SMB` | each governs the next memory reference |
| flow | `JMP JSX HLT INR EXIT` | |
| instruction | everything else the machine executes | |
| data, directive | `WORD TEXT ...`, `ORG EQU MACRO ...` | |
| macro call | anything else in the op field | `rexmac.asm`'s `JNZ`, `CALL`, ... without a list of them |

Operands: an `=`, `*` or `/` prefix, numbers (`12`, `0x1F`, `X'1F'`), character constants,
strings with their escapes (an unknown escape is marked as an error), `$`, `LEN(`, and
references to local labels.

## Generated files

`gen_syntax.py` writes `vim/` and `vscode/asm703/package.json` and
`vscode/asm703/syntaxes/asm703.tmLanguage.json` from the opcode and directive tables in
`asm703.py`. Edit the generator, not its output, and rerun it (`make -C tools/editor`)
after adding a mnemonic to the assembler. `make -C tools/editor check` fails when the
checked-in files are out of date, and CI runs it. `vscode/asm703/language-configuration.json`
(comment character, brackets, what a word is) is hand-written.
