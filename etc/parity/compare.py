#!/usr/bin/env python3
"""Compare pinned documented Git surface with compiled Brit parser exposure.

This is an inventory overlay, never a behavioral parity verdict. Boundary decisions
are authored governance; absent parser entries never imply intentional exclusion.
"""
import argparse
from collections import Counter
import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys

from atoms import command_atoms

ROOT = Path(__file__).resolve().parents[2]
DISPOSITIONS = {'undecided', 'planned', 'deferred', 'excluded', 'bridge-only', 'intentional-difference'}
TERMINATING = {'excluded', 'bridge-only'}


def declarations(path):
    """Read only the existing command table's claims; do not promote them to proof."""
    result = {}
    for line in path.read_text().splitlines():
        cells = [c.strip() for c in line.split('|')]
        if len(cells) > 4 and cells[3] in {'present', 'partial', 'absent', 'deferred'}:
            if cells[1] in result:
                raise ValueError('duplicate historical command declaration: ' + cells[1])
            result[cells[1]] = {'status': cells[3], 'declared_cli': cells[2],
                                'source': 'docs/parity/commands.md', 'evidence_kind': 'historical-declaration'}
    return result


def boundaries(document, names):
    if document.get('version') != 1:
        raise ValueError('unsupported boundary declaration version')
    result = {}
    for edge in document.get('commands', []):
        name = edge['command']
        if name not in names or name in result:
            raise ValueError('unknown or duplicate boundary command: ' + name)
        disposition = edge['disposition']
        if disposition not in DISPOSITIONS:
            raise ValueError('unknown boundary disposition: ' + disposition)
        if edge.get('decision') not in {'proposed', 'adopted'}:
            raise ValueError('boundary must distinguish proposed from adopted decisions')
        for field in ('owner', 'reason', 'alternative', 'reconsider_when'):
            if not isinstance(edge.get(field), str) or not edge[field].strip():
                raise ValueError(f'{name}: boundary needs {field}')
        result[name] = dict(edge, terminates=disposition in TERMINATING and edge['decision'] == 'adopted')
    return result


def option_names(command):
    names = set()
    for arg in command.get('options', []):
        for field, prefix in [('long', '--'), ('short', '-')]:
            if arg.get(field):
                names.add(prefix + arg[field])
        for field, prefix in [('longAliases', '--'), ('shortAliases', '-')]:
            names.update(prefix + alias for alias in arg.get(field, []))
    return names


def atomic_counts(groups):
    counts = Counter(group['parserNameMatch'] for group in groups)
    return {'optionGroups': len(groups),
            'groupsWithAllNames': counts['all'], 'groupsWithSomeNames': counts['some'],
            'groupsWithNoNames': counts['none'], 'groupsWithoutLiteralNames': counts['unresolved'],
            'optionNameOccurrences': sum(len(g['optionNames']) for g in groups),
            'optionNameOccurrencesMatched': sum(len(g['matchedNames']) for g in groups),
            'conditionalGroups': sum(g['applicability'] == 'conditional-unresolved' for g in groups),
            'groupsWithBehavioralEvidence': 0}


def compare_atoms(ref, parser_entry):
    groups = []
    exposed = option_names(parser_entry) if parser_entry else set()
    for group in command_atoms(ref)['groups']:
        if group['kind'] != 'option':
            continue
        names = set(group['optionNames'])
        matched = names & exposed
        state = 'unresolved' if not names else 'none' if not matched else 'all' if matched == names else 'some'
        arguments = [] if parser_entry is None else [arg for arg in parser_entry.get('options', [])
                     if option_names({'options': [arg]}) & names]
        groups.append(dict(group, matchedNames=sorted(matched), missingNames=sorted(names - exposed),
                           parserNameMatch=state, parserArguments=arguments,
                           parserPath=None if parser_entry is None else [parser_entry['name']],
                           scopeVerdict='documentation-to-executable-scope-unresolved',
                           behavioralVerdict='unmeasured', boundaryInheritedFrom=ref['id']))
    return groups


def select_command(report, name):
    rows = [row for row in report['commands'] if row['name'] == name]
    if not rows:
        raise ValueError('unknown reference command: ' + name)
    return dict(report, commands=rows, selectedCommand=name,
                countsScope='whole-reference; selected command counts are on its row')


def compare(census, surface, declared, decisions):
    if surface.get('schemaVersion') != 1 or surface.get('evidenceKind') != 'parser-surface':
        raise ValueError('expected version 1 compiled parser-surface evidence')
    if census.get('schemaVersion') != 1:
        raise ValueError('expected version 1 reference census')
    revision = census.get('source', {}).get('revision')
    if not isinstance(revision, str) or not re.fullmatch(r'[0-9a-f]{40}|[0-9a-f]{64}', revision):
        raise ValueError('reference revision must be a full immutable object ID')
    for field in ('sourceHead', 'sourceState', 'frontendPresets'):
        if not isinstance(surface.get(field), str) or not surface[field].strip():
            raise ValueError('parser evidence needs ' + field)
    commands = census['commands']
    names = {c['name'] for c in commands}
    if not names or len(names) != len(commands):
        raise ValueError('reference command inventory is empty or duplicated')
    edge_by_name = boundaries(decisions, names)
    public = surface['command']
    rows = []
    for ref in commands:
        name = ref['name']
        matches = [c for c in public['subcommands'] if name in [c['name'], *c.get('aliases', [])]]
        if len(matches) > 1:
            raise ValueError('ambiguous compiled parser entry: ' + name)
        exposed = option_names(matches[0]) if matches else set()
        documented = {n for t in ref.get('terms', []) for n in t.get('option_names', [])}
        conditional = {n for t in ref.get('terms', [])
                       if t.get('applicability') == 'conditional-unresolved'
                       for n in t.get('option_names', [])}
        edge = edge_by_name.get(name, {'command': name, 'disposition': 'undecided',
                                      'decision': None, 'terminates': False, 'owner': None,
                                      'reason': None, 'alternative': None, 'reconsider_when': None})
        groups = compare_atoms(ref, matches[0] if matches else None)
        rows.append({'optionGroups': groups, 'atomicCounts': atomic_counts(groups), 'id': ref['id'], 'name': name, 'categories': ref.get('categories', []),
                     'historical_declaration': declared.get(name),
                     'parser_entry': matches[0]['name'] if matches else None,
                     'documented_option_names': sorted(documented),
                     'conditional_documented_option_names': sorted(conditional),
                     'parser_option_name_matches': sorted(documented & exposed),
                     'documented_option_names_not_exposed': sorted(documented - exposed),
                     'behavioral_verdict': 'unmeasured', 'boundary': edge})
    matched = {r['parser_entry'] for r in rows if r['parser_entry']}
    return {'schemaVersion': 1, 'evidenceKind': 'surface-comparison-not-behavioral-parity',
            'countsScope': 'whole-reference',
            'atomicCounts': atomic_counts([g for row in rows for g in row['optionGroups']]),
            'reference': census['source'],
            'parser': {k: surface.get(k) for k in ['sourceHead', 'sourceState', 'frontendPresets']},
            'counts': {'referenceCommands': len(rows),
                       'commandsWithParserEntry': sum(r['parser_entry'] is not None for r in rows),
                       'commandsWithoutParserEntry': sum(r['parser_entry'] is None for r in rows),
                       'commandsWithBehavioralEvidenceInThisReport': 0,
                       'boundaryDispositions': dict(sorted(Counter(r['boundary']['disposition'] for r in rows).items())),
                       'adoptedTerminatingEdges': sum(r['boundary']['terminates'] for r in rows)},
            'commands': rows,
            'nativeOrUnmappedParserEntries': [c['name'] for c in public['subcommands'] if c['name'] not in matched],
            'historicalClaimsOutsideReference': sorted(set(declared) - names),
            'limitations': ['Parser presence and option-name matches do not establish behavior or acceptance.',
                           'Missing entries are undecided unless an authored boundary says otherwise.',
                           'Terminating edges remain in the reference denominator.',
                           'Matching is by exact public command name or accepted alias; engine APIs are not CLI coverage.',
                           'Conditional documentation and extraction diagnostics remain in the source census.',
                           'Nested verbs and global Git interface options are inventoried separately, not matched by this top-level overlay.',
                           'Atomic name matches are scoped to the exact top-level parser; descendant options never inflate parent matches.',
                           'All/some/none count syntactic names in a documentation group, not supported capability or semantic aliases.',
                           'No behavioral run receipts are consumed by this version; unmeasured is not failed.']}


def render(report):
    c = report['counts']
    lines = ['# Brit against the pinned Git command surface', '',
             f"Reference: `{report['reference']['revision']}`.",
             f"Parser: `{report['parser']['sourceHead']}` ({report['parser']['sourceState']}); presets `{report['parser']['frontendPresets']}`.", '',
             f"{c['referenceCommands']} reference commands; {c['commandsWithParserEntry']} parser entries; {c['commandsWithoutParserEntry']} without a matching parser entry.",
             'Behavioral parity is unmeasured by this report. Exclusions remain in the denominator.', '',
             f"Option groups: {report['atomicCounts']['optionGroups']}; all/some/no documented names matched: {report['atomicCounts']['groupsWithAllNames']}/{report['atomicCounts']['groupsWithSomeNames']}/{report['atomicCounts']['groupsWithNoNames']}; unresolved names: {report['atomicCounts']['groupsWithoutLiteralNames']}.",
             'These are whole-reference syntactic counts; option scope and behavior remain unqualified.', '',
             '| Git command | Brit parser | Groups all/some/none/unresolved | Historical claim | Boundary | Reason / reconsideration |',
             '|---|---|---|---|---|---|']
    def cell(value):
        return str(value).replace('|', '\\|').replace('\n', ' ')
    for row in report['commands']:
        edge = row['boundary']
        reason = 'No decision declared' if edge['reason'] is None else edge['reason'] + ' / Revisit: ' + edge['reconsider_when']
        a = row['atomicCounts']
        group_counts = '/'.join(str(a[k]) for k in ('groupsWithAllNames', 'groupsWithSomeNames', 'groupsWithNoNames', 'groupsWithoutLiteralNames'))
        lines.append('| ' + ' | '.join(cell(x) for x in [row['name'], row['parser_entry'] or 'not exposed', group_counts,
                     (row['historical_declaration'] or {}).get('status', 'uncatalogued'),
                     edge['disposition'] + (f" ({edge['decision']})" if edge['decision'] else ''), reason]) + ' |')
    if report.get('selectedCommand'):
        row = report['commands'][0]
        lines.extend(['', '## Option-group walk: ' + row['name'], '',
                      'Each group appears once; sections are documentation context, not executable scope.',
                      'The command boundary above applies to every group below.', ''])
        for group in row['optionGroups']:
            lines.append('### ' + ' / '.join(cell(label) for label in group['labels']))
            lines.extend(['', '- Context: `' + group['id'] + '`; shared definition: `' + group['sharedId'] + '`',
                          '- Documentation sections: ' + ', '.join(group['sections']),
                          '- Applicability: ' + group['applicability'],
                          '- Parser name match: ' + group['parserNameMatch'] + '; scope and behavior unqualified',
                          '- Matched names: ' + (', '.join(group['matchedNames']) or 'none'),
                          '- Missing names: ' + (', '.join(group['missingNames']) or 'none')])
            lines.append('- Source: ' + ', '.join(sorted({s['path'] + ':' + str(s['line']) for s in group['sources']})))
            lines.append('')
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--census', type=Path, default=ROOT / 'docs/parity/git-census.json')
    parser.add_argument('--boundaries', type=Path, default=ROOT / '.epr-meta/git-boundaries.json')
    parser.add_argument('--declarations', type=Path, default=ROOT / 'docs/parity/commands.md')
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--brit', type=Path, help='Execute this built Brit with --cli-surface-json')
    group.add_argument('--surface', type=Path, help='Previously captured parser JSON; provenance is caller-owned')
    parser.add_argument('--markdown', action='store_true')
    parser.add_argument('--command', help='Walk one reference command; whole-reference totals remain explicit')
    args = parser.parse_args()
    try:
        if args.brit:
            binary = args.brit.resolve(strict=True)
            data = subprocess.run([str(binary), '--cli-surface-json'], check=True, capture_output=True,
                                  text=True, timeout=15).stdout
            digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        else:
            data = args.surface.read_text()
            digest = None
        result = compare(json.loads(args.census.read_text()), json.loads(data),
                         declarations(args.declarations), json.loads(args.boundaries.read_text()))
        result['parser']['binarySha256'] = digest
        if args.command:
            result = select_command(result, args.command)
        print(render(result) if args.markdown else json.dumps(result, indent=2), end='\n')
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print('census comparison refused: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
