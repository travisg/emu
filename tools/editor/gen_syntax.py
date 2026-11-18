#!/usr/bin/env python3
# vim: ts=4:sw=4:expandtab:
#
# Copyright (c) 2026 Travis Geiselbrecht
#
# Use of this source code is governed by a MIT-style
# license that can be found in the LICENSE file or at
# https://opensource.org/licenses/MIT
"""Write the 703 assembly syntax highlighters from asm703.py's own tables.

    gen_syntax.py            regenerate the files below in place
    gen_syntax.py --check    fail if any of them differs from what it would write

Outputs, all checked in so an install needs no python:

    vim/syntax/asm703.vim                       the vim syntax
    vim/ftdetect/asm703.vim                     which .asm files are the 703's
    vscode/asm703/package.json                  the VS Code extension's manifest
    vscode/asm703/syntaxes/asm703.tmLanguage.json   its TextMate grammar

The mnemonics come from the assembler's opcode and directive tables, so a
mnemonic added there reaches both highlighters by rerunning this. What it
cannot know is a deck's macros: a statement is `LABEL OP OPERANDS ; comment`
with the label in column 1, so whatever stands in the op field and is not the
assembler's own is highlighted as a macro call -- rexmac.asm's JNZ and CALL,
and any macro written later, without a list of them.

The .asm extension belongs to every assembler, so a file is taken for the
703's by where it lives (FILE_GLOBS) rather than by its name.
"""

import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..'))
import asm703 as A  # noqa: E402

VERSION = '0.1.0'

# Where the tree keeps 703 source. Both editors take these as path globs.
FILE_GLOBS = ['rex/**/*.asm', 'test/703/**/*.asm']

# Each class of statement, its TextMate scope and the vim group it links to.
# Skips and page selections are classes of their own because each governs
# the word after it, which is where this machine's bugs live.
CLASSES = [
    # name       mnemonics                                            scope                                 vim
    ('Flow',     {'JMP', 'JSX', 'HLT', 'INR', 'EXIT'},                'keyword.control.flow',               'Statement'),
    ('Skip',     set(A.SKIPPERS),                                     'keyword.control.skip',               'Special'),
    ('Page',     set(A.PAGE_SELECTS),                                 'support.type.page',                  'Special'),
    ('Subr',     {'SUBR'},                                            'storage.type.function',              'Type'),
    ('Data',     set(A.DATA_OPS) - {'LPOOL'},                         'storage.type.data',                  'Type'),
    ('Directive', set(A.DIRECTIVES) - set(A.DATA_OPS) | {'LPOOL'},    'keyword.other.directive',            'PreProc'),
]
_taken = set().union(*(c[1] for c in CLASSES))
# Everything else the assembler knows is an ordinary instruction. The shifts
# are their own entry only because their stems may be followed by a separate
# D, L or R (SHIFT_STEMS), which is one mnemonic printed with a gap.
INSTRUCTIONS = (set(A.MEMREF) | set(A.GEN_NONE) | set(A.GEN_LEVEL) | set(A.GEN_PAGE)
                | set(A.GEN_LITERAL) | set(A.GEN_DIO) | set(A.GEN_TWOWORD)
                | set(A.GEN_ALIAS) | {'EXCH'}) - _taken - set(A.GEN_SHIFT)
CLASSES.append(('Instr', INSTRUCTIONS, 'keyword.other.instruction', 'Statement'))
SHIFTS = set(A.GEN_SHIFT) - _taken
CLASSES.append(('Shift', SHIFTS, 'keyword.other.instruction.shift', 'Statement'))

_all = set().union(*(c[1] for c in CLASSES))
_missing = (set(A.MEMREF) | set(A.GEN_SKIP) | set(A.GEN_SHIFT) | set(A.DIRECTIVES)
            | set(A.SUBR_MACROS) | {A.SELECT_BASE}) - _all
assert not _missing, f'mnemonics in no class: {sorted(_missing)}'


def longest_first(names):
    return sorted(names, key=lambda n: (-len(n), n))


# ---------------------------------------------------------------- TextMate

def textmate():
    end = r'(?=[ \t;]|$)'
    op_groups = []      # (scope, regex) in the order the alternation tries them
    for name, ops, scope, _ in CLASSES:
        alt = '|'.join(longest_first(ops))
        if name == 'Shift':
            stems = '|'.join(longest_first(A.SHIFT_STEMS))
            alt = rf'(?:{stems})[ \t]+[DLR]{end}|{alt}'
        op_groups.append((scope, alt))
    op_groups.append(('entity.name.function.macro', r'[A-Za-z_][\w.]*'))

    def oppart(first):
        """The op field and an operand's prefix, as groups from `first`."""
        alts = '|'.join(f'((?i:{rx}))' for _, rx in op_groups)
        caps = {str(first + i): {'name': f'{scope}.asm703'}
                for i, (scope, _) in enumerate(op_groups)}
        prefix = first + len(op_groups)
        caps[str(prefix)] = {'name': 'keyword.operator.prefix.asm703'}
        return rf'(?:{alts}){end}(?:[ \t]+([=*/]+))?', caps

    labelled_op, labelled_caps = oppart(3)
    bare_op, bare_caps = oppart(1)
    labelled_caps['1'] = {'name': 'entity.name.label.local.asm703'}
    labelled_caps['2'] = {'name': 'entity.name.function.label.asm703'}

    comment_body = [{'match': r'\b(?:TODO|FIXME|XXX)\b', 'name': 'keyword.todo.asm703'}]
    grammar = {
        '$schema': 'https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json',
        'name': 'Raytheon 703 assembly',
        'scopeName': 'source.asm703',
        'comment': 'Generated by tools/editor/gen_syntax.py from asm703.py -- edit that, not this.',
        'patterns': [
            # SYM II's comment card: a quote in column 1
            {'match': "^'.*$", 'name': 'comment.line.card.asm703'},
            # the label, then the op field: anchored, so it is tried first
            {'match': rf'^(?:(\.[^\s;]*)|([^\s;\'.][^\s;]*))(?:[ \t]+{labelled_op})?',
             'captures': labelled_caps},
            {'match': rf'^[ \t]+{bare_op}', 'captures': bare_caps},
            {'include': '#operand'},
        ],
        'repository': {
            'operand': {'patterns': [
                {'begin': ';', 'end': '$', 'name': 'comment.line.semicolon.asm703',
                 'patterns': comment_body},
                {'begin': '"', 'end': '"', 'name': 'string.quoted.double.asm703',
                 'patterns': [
                     {'match': r'\\[rnt0\\"]', 'name': 'constant.character.escape.asm703'},
                     {'match': r'\\.', 'name': 'invalid.illegal.escape.asm703'},
                 ]},
                {'match': r"\bX'[0-9A-Fa-f]+'", 'name': 'constant.numeric.hex.asm703'},
                {'match': r'\b0[xX][0-9A-Fa-f]+\b', 'name': 'constant.numeric.hex.asm703'},
                {'match': r"'[^']{1,2}'", 'name': 'string.quoted.single.asm703'},
                {'match': r'\b[0-9]+\b', 'name': 'constant.numeric.decimal.asm703'},
                {'match': r'\$', 'name': 'variable.language.location.asm703'},
                {'match': r'\bLEN(?=\()', 'name': 'support.function.builtin.asm703'},
                {'match': r'(?<![\w.])\.[A-Za-z_]\w*', 'name': 'variable.other.label.local.asm703'},
                {'match': r'[A-Za-z_][\w.]*', 'name': 'variable.other.asm703'},
                {'match': r'<=|>=|<>|[-+*/=<>]', 'name': 'keyword.operator.asm703'},
                {'match': ',', 'name': 'punctuation.separator.asm703'},
            ]},
        },
    }
    return grammar


def package():
    return {
        'name': 'asm703',
        'displayName': 'Raytheon 703 assembly',
        'description': 'Syntax highlighting for the asm703.py dialect of Raytheon 703 assembly.',
        'version': VERSION,
        'publisher': 'local',
        'license': 'MIT',
        'engines': {'vscode': '^1.60.0'},
        'categories': ['Programming Languages'],
        'contributes': {
            'languages': [{
                'id': 'asm703',
                'aliases': ['Raytheon 703 assembly', 'asm703'],
                'filenamePatterns': [f'**/{g}' for g in FILE_GLOBS],
                'configuration': './language-configuration.json',
            }],
            'grammars': [{
                'language': 'asm703',
                'scopeName': 'source.asm703',
                'path': './syntaxes/asm703.tmLanguage.json',
            }],
        },
    }


# --------------------------------------------------------------------- vim

def vim_syntax():
    # An op is the first word after the line's leading space, with or
    # without a label before it: a lookbehind, so the match never depends on
    # where the label's match left off.
    at_op = r'\%(^\S*\s\+\)\@<='
    end = r'\%(\s\|;\|$\)\@='
    lines = [
        '" vim: ts=8:sw=8:noexpandtab:',
        '" Vim syntax file',
        '" Language:	Raytheon 703 assembly, the asm703.py dialect',
        '" Generated by tools/editor/gen_syntax.py from asm703.py -- edit that, not this.',
        '',
        'if exists("b:current_syntax")',
        '  finish',
        'endif',
        '',
        'syn case ignore',
        '',
        '" the op field: a macro call unless one of the classes below claims it',
        rf'syn match asm703Macro /{at_op}\h[0-9A-Za-z_.]*{end}/',
    ]
    for name, ops, _, _ in CLASSES:
        alt = r'\|'.join(longest_first(ops))
        if name == 'Shift':
            stems = r'\|'.join(longest_first(A.SHIFT_STEMS))
            alt = rf'\%({stems}\)\s\+[DLR]{end}\|{alt}'
        lines.append(rf'syn match asm703{name} /{at_op}\%({alt}\){end}/')
    lines += [
        '',
        '" an operand\'s prefix: = literal, * indexed, / byte address',
        rf'syn match asm703Prefix /\%(^\S*\s\+\S\+\s\+\)\@<=[=*/]\+/',
        '',
        'syn case match',
        '',
        '" a reference first: at column 1 the label is the later match, and wins',
        r'syn match asm703LocalRef /\%([0-9A-Za-z_.]\)\@<!\.\h\w*/',
        r'syn match asm703Label /^[^ \t;' + "'" + r'.][^ \t;]*/',
        r'syn match asm703LocalLabel /^\.[^ \t;]*/',
        '',
        r'syn match asm703Decimal /\<\d\+\>/',
        r'syn match asm703Hex /\<0[xX]\x\+\>/',
        r"syn match asm703Hex /\<X'\x\+'/",
        r"syn match asm703Char /'[^']\{1,2}'/",
        r'syn match asm703Here /\$/',
        r'syn match asm703Builtin /\<LEN\ze(/',
        r'syn match asm703BadEscape /\\./ contained',
        r'syn match asm703Escape /\\[rnt0\\"]/ contained',
        r'syn region asm703String start=/"/ skip=/\\./ end=/"/ contains=asm703Escape,asm703BadEscape oneline',
        '',
        'syn keyword asm703Todo contained TODO FIXME XXX',
        'syn match asm703Comment /;.*/ contains=asm703Todo',
        "syn match asm703CommentCard /^'.*/ contains=asm703Todo",
        '',
    ]
    links = [(f'asm703{name}', vim) for name, _, _, vim in CLASSES] + [
        ('asm703Macro', 'Macro'),
        ('asm703Prefix', 'SpecialChar'),
        ('asm703Label', 'Function'),
        ('asm703LocalLabel', 'Identifier'),
        ('asm703LocalRef', 'Identifier'),
        ('asm703Decimal', 'Number'),
        ('asm703Hex', 'Number'),
        ('asm703Char', 'Character'),
        ('asm703Here', 'Special'),
        ('asm703Builtin', 'Function'),
        ('asm703Escape', 'SpecialChar'),
        ('asm703BadEscape', 'Error'),
        ('asm703String', 'String'),
        ('asm703Todo', 'Todo'),
        ('asm703Comment', 'Comment'),
        ('asm703CommentCard', 'Comment'),
    ]
    lines += [f'hi def link {a:<18} {b}' for a, b in links]
    lines += ['', 'let b:current_syntax = "asm703"', '']
    return '\n'.join(lines)


def vim_ftdetect():
    # vim's own *.asm detection has already run by the time ftdetect's
    # autocommands do, so this sets the filetype rather than `setf`, which
    # would leave the first answer standing.
    pats = ','.join('*/' + g.replace('**/', '') for g in FILE_GLOBS)
    return ('" vim: ts=8:sw=8:noexpandtab:\n'
            '" Generated by tools/editor/gen_syntax.py -- edit that, not this.\n'
            f'au BufRead,BufNewFile {pats} set filetype=asm703\n')


def outputs():
    return {
        'vim/syntax/asm703.vim': vim_syntax(),
        'vim/ftdetect/asm703.vim': vim_ftdetect(),
        'vscode/asm703/package.json': json.dumps(package(), indent=2) + '\n',
        'vscode/asm703/syntaxes/asm703.tmLanguage.json': json.dumps(textmate(), indent=2) + '\n',
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--check', action='store_true',
                    help='report files that differ from what would be written, and fail')
    args = ap.parse_args()
    stale = []
    for rel, text in outputs().items():
        path = os.path.join(HERE, rel)
        try:
            with open(path, encoding='utf-8') as f:
                current = f.read()
        except FileNotFoundError:
            current = None
        if current == text:
            continue
        if args.check:
            stale.append(rel)
            continue
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, 'w', encoding='utf-8') as f:
            f.write(text)
        print(f'wrote {rel}')
    if stale:
        print('out of date with asm703.py (run tools/editor/gen_syntax.py):', *stale, sep='\n  ')
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
