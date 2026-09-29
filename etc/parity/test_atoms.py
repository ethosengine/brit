"""Documentation graph identity, provenance and denominator regressions."""
from copy import deepcopy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import atoms
from test_census import SourceFixture


def term(identifier='shared', names=None, exact=True):
    source = {'path': 'Documentation/shared.adoc', 'line': 3,
              'conditions': [{'kind': 'ifdef', 'expression': 'demo'}], 'via': []}
    if exact:
        source.update(section='OPTIONS', section_source={'path': 'Documentation/demo.adoc', 'line': 1})
    return {'id': identifier, 'kind': 'option', 'labels': ['--quiet', '-q'],
            'option_names': names if names is not None else ['--quiet', '-q'],
            'sections': ['OPTIONS'], 'sources': [source], 'applicability': 'conditional-unresolved'}


def command(name='demo', terms=None):
    return {'id': 'git:command:' + name, 'name': name, 'terms': terms if terms is not None else [term()]}


def census(commands=None):
    return {'schemaVersion': 1, 'source': {'revision': 'fixture'},
            'commands': commands if commands is not None else [command()], 'interfaces': [], 'diagnostics': []}


class AtomTests(unittest.TestCase):
    def test_shared_groups_and_context_names_are_distinct_units(self):
        result = atoms.graph(census([command(), command('other')]))
        self.assertEqual(result['counts']['uniqueSharedGroups'], 1)
        self.assertEqual(result['counts']['commandOccurrences']['groups'], 2)
        self.assertEqual(result['counts']['commandOccurrences']['optionNameAtoms'], 4)
        self.assertEqual(result['counts']['uniqueSharedOptionNames'], 2)
        self.assertNotEqual(result['commands'][0]['groups'][0]['id'], result['commands'][1]['groups'][0]['id'])

    def test_duplicate_includes_do_not_inflate_groups_or_alias_occurrences(self):
        first = term()
        second = deepcopy(first)
        second['sources'][0].update(section='OTHER', section_source={'path': 'Documentation/demo.adoc', 'line': 20})
        second['sections'] = ['OTHER']
        ref = command(terms=[first, first, second, term('second')])
        original = deepcopy(ref)
        result = atoms.command_atoms(ref)
        self.assertEqual(result['counts']['groups'], 2)
        self.assertEqual(result['counts']['optionNameAtoms'], 2)
        self.assertEqual(result['counts']['optionNameOccurrences'], 4)
        group = next(g for g in result['groups'] if g['sharedId'] == 'shared')
        self.assertEqual(group['sections'], ['OPTIONS', 'OTHER'])
        self.assertEqual(len(group['sources']), 2)
        self.assertTrue(all('sectionId' in s and s['conditions'] for s in group['sources']))
        self.assertEqual(ref, original)

    def test_legacy_sections_remain_aggregate_only(self):
        legacy = term(exact=False)
        legacy['sections'] = ['ONE', 'TWO']
        result = atoms.command_atoms(command(terms=[legacy]))
        group = result['groups'][0]
        self.assertEqual(group['sectionAssociation'], 'aggregate-only')
        self.assertEqual(group['sections'], ['ONE', 'TWO'])
        self.assertNotIn('sectionId', group['sources'][0])
        self.assertTrue(all(s['association'] == 'aggregate-only' for s in result['sections']))

    def test_scope_is_never_inferred_from_documentation(self):
        verb = term(names=[])
        verb['kind'] = 'nested-verb'
        result = atoms.command_atoms(command(terms=[verb]))
        self.assertEqual(result['groups'][0]['kind'], 'nested-verb-candidate')
        self.assertEqual(result['groups'][0]['executableScope'], 'unresolved')
        self.assertEqual(result['counts']['optionGroups'], 0)
        moved = deepcopy(verb)
        moved['sources'][0]['section_source']['line'] += 100
        self.assertEqual(result['sections'][0]['id'], atoms.command_atoms(command(terms=[moved]))['sections'][0]['id'])

    def test_conflicts_refuse_and_context_order_is_deterministic(self):
        with self.assertRaises(ValueError):
            atoms.graph(census([command(), command()]))
        with self.assertRaises(ValueError):
            atoms.graph(census([command(), command('other', [term(names=['--different'])])]))
        self.assertEqual(atoms.graph(census([command(), command('other')])), atoms.graph(census([command('other'), command()])))

    def test_source_sections_and_wide_literal_blocks(self):
        fixture = SourceFixture()
        self.addCleanup(fixture.close)
        fixture.write('Documentation/git-demo.adoc', 'FIRST\n=====\ninclude::shared.adoc[]\nSECOND\n======\ninclude::shared.adoc[]\n\n------------------------------------------------------------------------\n  I---X\n--------\n--fake::\n------------------------------------------------------------------------\n')
        fixture.write('Documentation/shared.adoc', '--shared::\n Description.\n')
        result = fixture.build()
        terms = result['commands'][0]['terms']
        self.assertEqual(len(terms), 1)
        self.assertEqual({s['section'] for s in terms[0]['sources']}, {'FIRST', 'SECOND'})
        self.assertEqual({s['section_source']['line'] for s in terms[0]['sources']}, {1, 4})
        self.assertFalse(any(d['code'] == 'unclosed-literal-block' for d in result['diagnostics']))

    def test_cli_json_tree_and_unknown_context(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'census.json'
            path.write_text(json.dumps(census()))
            argv = [sys.executable, str(Path(atoms.__file__)), '--census', str(path), '--command', 'demo']
            tree = subprocess.run(argv, capture_output=True, text=True)
            self.assertEqual(tree.returncode, 0, tree.stderr)
            self.assertIn('name --quiet', tree.stdout)
            document = subprocess.run([*argv, '--json'], capture_output=True, text=True)
            self.assertEqual(json.loads(document.stdout)['context']['counts']['groups'], 1)
            bad = subprocess.run([*argv[:-1], 'absent'], capture_output=True, text=True)
            self.assertEqual(bad.returncode, 2)

    def test_real_census_denominators_and_edges(self):
        path = Path(atoms.ROOT) / 'docs/parity/git-census.json'
        data = json.loads(path.read_text())
        result = atoms.graph(data)
        expected = sum(len({t['id'] for t in c['terms']}) for c in data['commands'])
        self.assertEqual(result['counts']['commandOccurrences']['groups'], expected)
        expected_names = sum(len(t['option_names']) for c in data['commands'] for t in c['terms'])
        self.assertEqual(result['counts']['commandOccurrences']['optionNameOccurrences'], expected_names)
        self.assertFalse(any(s['label'] == 'I---X' for c in result['commands'] for s in c['sections']))
        nodes = {c['id'] for c in result['commands'] + result['interfaces']}
        for c in result['commands'] + result['interfaces']:
            nodes.update(n['id'] for key in ('groups', 'sections', 'nameAtoms') for n in c[key])
        nodes.update(n['id'] for key in ('sharedGroups', 'sharedNames') for n in result[key])
        self.assertTrue(all(e['from'] in nodes and e['to'] in nodes for e in result['edges']))


if __name__ == '__main__':
    unittest.main()
