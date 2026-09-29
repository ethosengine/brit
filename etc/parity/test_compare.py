import copy
import unittest
import compare as comparison


class ComparisonTests(unittest.TestCase):
    def setUp(self):
        self.census = {'schemaVersion': 1, 'source': {'revision': 'a'*40}, 'commands': [
            {'id': 'git:command:add', 'name': 'add', 'terms': [{'id': 'git:term:fixture', 'kind': 'option', 'labels': ['-n', '--dry-run'],
                'option_names': ['-n', '--dry-run'], 'sections': ['OPTIONS'],
                'sources': [{'path': 'Documentation/git-add.adoc', 'line': 1, 'conditions': [], 'via': []}],
                'applicability': 'documented-unconditional'}]},
            {'id': 'git:command:send-email', 'name': 'send-email', 'terms': []}]}
        self.surface = {'schemaVersion': 1, 'evidenceKind': 'parser-surface',
                        'sourceHead': 'b'*40, 'sourceState': 'dirty', 'frontendPresets': 'default',
                        'command': {'name': 'brit', 'subcommands': [
                            {'name': 'stage', 'aliases': ['add'], 'options': [{'long': 'dry-run', 'short': 'n'}]},
                            {'name': 'snapshot', 'aliases': [], 'options': []}]}}
        self.policy = {'version': 1, 'commands': []}

    def result(self):
        return comparison.compare(self.census, self.surface, {'add': {'status': 'present'}}, self.policy)

    def test_alias_parser_presence_is_never_behavioral_proof(self):
        r = self.result()
        self.assertEqual(r['counts']['commandsWithParserEntry'], 1)
        self.assertEqual(r['counts']['commandsWithBehavioralEvidenceInThisReport'], 0)
        self.assertEqual(r['commands'][0]['parser_option_name_matches'], ['--dry-run', '-n'])
        self.assertEqual(r['commands'][0]['behavioral_verdict'], 'unmeasured')
        self.assertEqual(r['nativeOrUnmappedParserEntries'], ['snapshot'])

    def test_atomic_groups_count_names_without_promoting_behavior(self):
        r = self.result()
        self.assertEqual(r['atomicCounts']['optionGroups'], 1)
        self.assertEqual(r['atomicCounts']['groupsWithAllNames'], 1)
        self.assertEqual(r['atomicCounts']['optionNameOccurrencesMatched'], 2)
        group = r['commands'][0]['optionGroups'][0]
        self.assertEqual(group['boundaryInheritedFrom'], 'git:command:add')
        self.assertEqual(group['behavioralVerdict'], 'unmeasured')
        self.assertIn('unresolved', group['scopeVerdict'])
        self.assertEqual(group['parserArguments'][0]['long'], 'dry-run')
        selected = comparison.select_command(r, 'add')
        self.assertEqual(selected['counts']['referenceCommands'], 2)
        self.assertIn('whole-reference', selected['countsScope'])
        self.assertIn('Option-group walk: add', comparison.render(selected))
        with self.assertRaisesRegex(ValueError, 'unknown reference'):
            comparison.select_command(r, 'typo')

    def test_partial_missing_parameterized_and_non_option_groups(self):
        base = self.census['commands'][0]['terms'][0]
        self.surface['command']['subcommands'][0]['options'][0].pop('short')
        parameterized = dict(copy.deepcopy(base), id='git:term:parameter', labels=['--<field>'], option_names=[])
        missing = dict(copy.deepcopy(base), id='git:term:missing', labels=['--missing'], option_names=['--missing'])
        definition = dict(copy.deepcopy(base), id='git:term:definition', kind='definition', labels=['path'], option_names=[])
        self.census['commands'][0]['terms'] += [parameterized, missing, definition]
        a = self.result()['atomicCounts']
        self.assertEqual((a['optionGroups'], a['groupsWithAllNames'], a['groupsWithSomeNames'],
                          a['groupsWithNoNames'], a['groupsWithoutLiteralNames']), (3, 0, 1, 1, 1))

    def test_descendant_option_does_not_match_parent_scope(self):
        command = self.surface['command']['subcommands'][0]
        command['subcommands'] = [{'name': 'nested', 'options': command.pop('options')}]
        self.assertEqual(self.result()['atomicCounts']['groupsWithNoNames'], 1)

    def test_missing_command_and_exclusion_keep_atomic_denominator(self):
        self.census['commands'][1]['terms'] = copy.deepcopy(self.census['commands'][0]['terms'])
        self.policy['commands'] = [{'command': 'send-email', 'disposition': 'excluded',
            'decision': 'adopted', 'owner': 'fixture', 'reason': 'fixture boundary',
            'alternative': 'fixture bridge', 'reconsider_when': 'fixture evidence'}]
        r = self.result()
        self.assertEqual(r['atomicCounts']['optionGroups'], 2)
        self.assertEqual(r['atomicCounts']['groupsWithNoNames'], 1)
        self.assertEqual(r['commands'][1]['optionGroups'][0]['boundaryInheritedFrom'], 'git:command:send-email')
        self.assertNotEqual(r['commands'][0]['optionGroups'][0]['id'], r['commands'][1]['optionGroups'][0]['id'])

    def test_conditional_options_remain_identifiable(self):
        self.census['commands'][0]['terms'][0]['applicability'] = 'conditional-unresolved'
        self.assertEqual(self.result()['atomicCounts']['conditionalGroups'], 1)
        self.assertEqual(self.result()['commands'][0]['conditional_documented_option_names'],
                         ['--dry-run', '-n'])

    def test_missing_command_is_undecided_not_excluded(self):
        edge = self.result()['commands'][1]['boundary']
        self.assertEqual(edge['disposition'], 'undecided')
        self.assertFalse(edge['terminates'])

    def test_adopted_exclusion_is_reasoned_edge_not_denominator_removal(self):
        self.policy['commands'] = [{'command': 'send-email', 'disposition': 'excluded',
            'decision': 'adopted', 'owner': 'fixture-cli', 'reason': 'Fixture policy: mail is external',
            'alternative': 'Use explicit legacy Git bridge', 'reconsider_when': 'Native mail workflow admitted'}]
        r = self.result()
        self.assertEqual(r['counts']['referenceCommands'], 2)
        self.assertEqual(r['counts']['adoptedTerminatingEdges'], 1)
        self.assertIn('Native mail workflow', comparison.render(r))
        self.policy['commands'][0]['decision'] = 'proposed'
        self.assertEqual(self.result()['counts']['adoptedTerminatingEdges'], 0)

    def test_reasonless_or_unknown_boundary_refuses(self):
        self.policy['commands'] = [{'command': 'send-email', 'disposition': 'excluded', 'decision': 'adopted'}]
        with self.assertRaisesRegex(ValueError, 'owner'):
            self.result()
        self.policy['commands'][0]['command'] = 'not-in-reference'
        with self.assertRaisesRegex(ValueError, 'unknown'):
            self.result()

    def test_unknown_schema_or_missing_provenance_refuses(self):
        self.census['schemaVersion'] = 2
        with self.assertRaisesRegex(ValueError, 'version 1 reference'):
            self.result()
        self.census['schemaVersion'] = 1
        self.census['source']['revision'] = 'main'
        with self.assertRaisesRegex(ValueError, 'immutable'):
            self.result()
        self.census['source']['revision'] = 'a'*40
        self.surface['sourceState'] = ''
        with self.assertRaisesRegex(ValueError, 'sourceState'):
            self.result()

    def test_ambiguous_surface_and_duplicate_reference_refuse(self):
        self.surface['command']['subcommands'].append(copy.deepcopy(self.surface['command']['subcommands'][0]))
        with self.assertRaisesRegex(ValueError, 'ambiguous'):
            self.result()
        self.census['commands'].append(copy.deepcopy(self.census['commands'][0]))
        with self.assertRaisesRegex(ValueError, 'duplicated'):
            self.result()


if __name__ == '__main__':
    unittest.main()
