#!/usr/bin/env python3
"""Derive a walkable documentation graph from the pinned census; no second catalog.

Context groups, shared definitions and option spellings are different count units.
Sections and nested-verb candidates describe documentation, never executable scope.
"""
from __future__ import annotations

import argparse
from collections import Counter
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[2]


def stable_key(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()[:24]


def unique(rows):
    """Deterministic structural union, used for provenance and repeated include edges."""
    return [value for _, value in sorted({json.dumps(row, sort_keys=True): row for row in rows}.items())]


def command_atoms(ref):
    """One command/interface context, without claiming any executable subcommand scope."""
    owner = ref['id']
    groups, sections, names = {}, {}, {}
    for term in ref.get('terms', []):
        shared = term['id']
        identifier = owner + '/group/' + quote(shared, safe='')
        kind = 'nested-verb-candidate' if term['kind'] == 'nested-verb' else term['kind']
        spellings = sorted(set(term.get('option_names', [])))
        labels = list(term.get('labels', []))
        group = groups.get(identifier)
        if group and (group['labels'] != labels or group['optionNames'] != spellings or group['kind'] != kind):
            raise ValueError('conflicting definition group: ' + identifier)
        if group is None:
            group = groups[identifier] = {
                'id': identifier, 'sharedId': shared, 'kind': kind, 'labels': labels,
                'optionNames': spellings, 'nameAtomIds': [], 'sectionIds': [], 'sources': [],
                'applicability': term.get('applicability', 'unresolved'),
                'executableScope': 'unresolved', 'sectionAssociation': 'per-occurrence',
            }
        elif group['applicability'] != term.get('applicability', 'unresolved'):
            group['applicability'] = 'unresolved'
        occurrences = deepcopy(term.get('sources', []))
        # Older census bytes have only two independent unions: labels and sources.
        # Never invent a source/section cross-product when adapting those bytes.
        exact = [source for source in occurrences if 'section' in source]
        missing = [source for source in occurrences if 'section' not in source]
        contexts = [(source.get('section', ''), source.get('section_source'), 'per-occurrence', source) for source in exact]
        if missing or not occurrences:
            group['sectionAssociation'] = 'aggregate-only' if not exact else 'mixed'
            contexts.extend((label, None, 'aggregate-only', None) for label in term.get('sections', []))
        if not contexts:
            contexts.append(('', None, 'unresolved', None))
        for label, origin, association, source in contexts:
            section_id = owner + '/section/' + stable_key([label, (origin or {}).get('path'), association])
            section = sections.setdefault(section_id, {'id': section_id, 'label': label or '(section unspecified)',
                'association': association, 'sources': [], 'groupIds': [], 'executableScope': 'unresolved'})
            if origin:
                section['sources'] = unique([*section['sources'], origin])
            if identifier not in section['groupIds']:
                section['groupIds'].append(identifier)
            if section_id not in group['sectionIds']:
                group['sectionIds'].append(section_id)
            # Exact per-occurrence binding remains on the occurrence itself.
            if source is not None:
                source['sectionId'] = section_id
        # deepcopy kept the records we just decorated independent from caller input.
        group['sources'] = unique([*group['sources'], *occurrences])
        for spelling in spellings:
            name_id = owner + '/option/' + quote(spelling, safe='')
            atom = names.setdefault(name_id, {'id': name_id, 'sharedNameId': 'git:option-name:' + quote(spelling, safe=''),
                'name': spelling, 'groupIds': [], 'executableScope': 'unresolved'})
            if identifier not in atom['groupIds']:
                atom['groupIds'].append(identifier)
            if name_id not in group['nameAtomIds']:
                group['nameAtomIds'].append(name_id)
    for group in groups.values():
        group['sectionIds'].sort()
        group['sections'] = sorted({sections[key]['label'] for key in group['sectionIds']})
        exact = sum('section' in source for source in group['sources'])
        group['sectionAssociation'] = ('per-occurrence' if exact == len(group['sources']) and exact else 'mixed' if exact else 'aggregate-only')
        group['nameAtomIds'].sort()
    for entry in [*sections.values(), *names.values()]:
        entry['groupIds'].sort()
    ordered_groups = [groups[key] for key in sorted(groups)]
    return {'id': owner, 'name': ref['name'], 'categories': ref.get('categories', []),
            'documentation': ref.get('documentation'), 'declaration': ref.get('declaration'),
            'groups': ordered_groups, 'nameAtoms': [names[key] for key in sorted(names)],
            'sections': [sections[key] for key in sorted(sections)],
            'counts': {'groups': len(groups), 'optionGroups': sum(g['kind'] == 'option' for g in ordered_groups),
                       'nestedVerbCandidates': sum(g['kind'] == 'nested-verb-candidate' for g in ordered_groups),
                       'otherDefinitions': sum(g['kind'] == 'definition' for g in ordered_groups),
                       'optionNameAtoms': len(names), 'optionNameOccurrences': sum(len(g['optionNames']) for g in ordered_groups),
                       'sections': len(sections)}}


def graph(census):
    if census.get('schemaVersion') != 1:
        raise ValueError('expected version 1 source census')
    commands = [command_atoms(ref) for ref in census['commands']]
    interfaces = [command_atoms(ref) for ref in census.get('interfaces', [])]
    contexts = [*commands, *interfaces]
    if len({context['id'] for context in contexts}) != len(contexts):
        raise ValueError('duplicate command/interface context identity')
    shared, spellings, edges = {}, {}, []
    for context in contexts:
        for section in context['sections']:
            edges.append({'from': context['id'], 'to': section['id'], 'kind': 'documents-section'})
            edges.extend({'from': section['id'], 'to': group_id, 'kind': 'mentions-group', 'association': section['association']} for group_id in section['groupIds'])
        for group in context['groups']:
            definition = shared.setdefault(group['sharedId'], {'id': group['sharedId'], 'labels': group['labels'],
                'optionNames': group['optionNames'], 'contextGroupIds': [], 'kinds': []})
            if definition['labels'] != group['labels'] or definition['optionNames'] != group['optionNames']:
                raise ValueError('inconsistent shared definition: ' + group['sharedId'])
            definition['contextGroupIds'].append(group['id'])
            if group['kind'] not in definition['kinds']:
                definition['kinds'].append(group['kind'])
            edges.extend([{'from': context['id'], 'to': group['id'], 'kind': 'documents-group'},
                          {'from': group['id'], 'to': group['sharedId'], 'kind': 'instance-of'}])
            edges.extend({'from': group['id'], 'to': name_id, 'kind': 'has-option-spelling'} for name_id in group['nameAtomIds'])
        for atom in context['nameAtoms']:
            spelling = spellings.setdefault(atom['sharedNameId'], {'id': atom['sharedNameId'], 'name': atom['name'], 'contextNameIds': []})
            spelling['contextNameIds'].append(atom['id'])
            edges.append({'from': atom['id'], 'to': atom['sharedNameId'], 'kind': 'spelling-of'})
    for entry in shared.values():
        entry['contextGroupIds'].sort()
        entry['kinds'].sort()
    for entry in spellings.values():
        entry['contextNameIds'].sort()
    def sum_counts(rows):
        totals = Counter()
        for row in rows:
            totals.update(row['counts'])
        return dict(sorted(totals.items()))
    return {'schemaVersion': 1, 'evidenceKind': 'documented-feature-atoms-not-behavioral-parity',
            'reference': census.get('source', {}), 'commands': sorted(commands, key=lambda c: c['id']),
            'interfaces': sorted(interfaces, key=lambda c: c['id']),
            'sharedGroups': [shared[key] for key in sorted(shared)],
            'sharedNames': [spellings[key] for key in sorted(spellings)], 'edges': unique(edges),
            'counts': {'commandContexts': len(commands), 'interfaceContexts': len(interfaces),
                       'commandOccurrences': sum_counts(commands), 'interfaceOccurrences': sum_counts(interfaces),
                       'uniqueSharedGroups': len(shared), 'uniqueSharedOptionNames': len(spellings)},
            'diagnostics': census.get('diagnostics', []),
            'countConventions': [
                'One context group per command/interface and shared definition ID, regardless of repeated includes or section edges.',
                'One name atom per command/interface and exact option spelling; aliases are separate spellings, not separate capability groups.',
                'Name occurrences count group-to-spelling membership; a name shared by two groups contributes two occurrences but one context name atom.',
                'Shared group and spelling totals deduplicate across commands and interfaces; command and interface occurrence totals remain separate.',
                'Section associations and nested-verb candidates establish documentation context only. Executable scope and behavior remain unresolved.',
                'Legacy census sources without per-occurrence sections retain aggregate-only associations; no source/section cross-product is inferred.']}


def render_command(command):
    counts = command['counts']
    lines = [f"{command['name']} [{command['id']}]", f"  {counts['optionGroups']} option groups; {counts['optionNameAtoms']} distinct name atoms; {counts['nestedVerbCandidates']} nested-verb candidates; {counts['otherDefinitions']} other definitions",
             '  Documentation context only; executable scope and behavioral parity are unresolved.']
    groups = {group['id']: group for group in command['groups']}
    rendered = set()
    for section in command['sections']:
        lines.append(f"  section {section['label']} ({section['association']})")
        for identifier in section['groupIds']:
            group = groups[identifier]
            if identifier in rendered:
                lines.append(f"    reference {identifier} (already shown; not another group)")
                continue
            rendered.add(identifier)
            lines.append(f"    {group['kind']}: {' | '.join(group['labels'])} [{group['applicability']}]")
            lines.append(f"      group {identifier}; shared {group['sharedId']}")
            for name in group['optionNames']:
                lines.append(f"      name {name}")
            for source in group['sources']:
                guards = ', '.join(f"{g['kind']}::{g['expression']}" for g in source.get('conditions', []))
                lines.append(f"      source {source.get('path')}:{source.get('line')}" + (f"; guards {guards}" if guards else ''))
    return '\n'.join(lines) + '\n'


def render(result):
    lines = [f"Git documentation atoms at {result['reference'].get('revision', 'unknown')}",
             json.dumps(result['counts'], sort_keys=True), '',
             'Use --command NAME or --interface NAME to walk definitions, aliases and source guards.', '']
    for label in ('commands', 'interfaces'):
        lines.append(label.upper())
        for row in result[label]:
            lines.append(f"  {row['name']}: {row['counts']['groups']} groups, {row['counts']['optionGroups']} option groups, {row['counts']['optionNameAtoms']} name atoms")
    return '\n'.join(lines) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--census', type=Path, default=ROOT / 'docs/parity/git-census.json')
    scope = parser.add_mutually_exclusive_group()
    scope.add_argument('--command')
    scope.add_argument('--interface')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args(argv)
    try:
        census = json.loads(args.census.read_text())
        if census.get('schemaVersion') != 1:
            raise ValueError('expected version 1 source census')
        name = args.command or args.interface
        if name:
            rows = census['commands'] if args.command else census.get('interfaces', [])
            matches = [row for row in rows if name in (row['name'], row['id'])]
            if len(matches) != 1:
                raise ValueError('unknown or ambiguous documentation context: ' + name)
            context = command_atoms(matches[0])
            result = {'schemaVersion': 1, 'evidenceKind': 'documented-feature-atoms-not-behavioral-parity',
                      'reference': census.get('source', {}), 'context': context}
            print(json.dumps(result, indent=2) if args.json else render_command(context), end='\n' if args.json else '')
        else:
            result = graph(census)
            print(json.dumps(result, indent=2) if args.json else render(result), end='\n' if args.json else '')
        return 0
    except (ValueError, KeyError, TypeError, OSError) as error:
        print('documentation graph refused: ' + str(error), file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
