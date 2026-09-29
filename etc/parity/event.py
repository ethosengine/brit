#!/usr/bin/env python3
"""Append a witnessed Bash helper execution; no source inference or verdict promotion."""
import json
import os
import sys


def main():
    phase, helper, mode, status, actual, git, expected, reason, line, invocation, *argv = sys.argv[1:]
    optional_int = lambda value: int(value) if value else None
    row = {
        'schemaVersion': 1, 'event': phase, 'suite': os.environ['PARITY_SUITE'],
        'suiteSha256': os.environ['PARITY_SUITE_SHA256'],
        'hashKind': os.environ.get('GIX_TEST_FIXTURE_HASH', 'sha1'),
        'title': os.environ.get('PARITY_TITLE', ''), 'it': os.environ.get('PARITY_IT', ''),
        'line': int(line), 'helper': helper, 'mode': mode, 'status': status,
        'invocationId': invocation, 'argv': argv, 'actualExit': optional_int(actual),
        'gitExit': optional_int(git), 'expectedExit': optional_int(expected),
        'reason': reason or None, 'compatReason': os.environ.get('PARITY_COMPAT_REASON') or None, 'binary': os.environ.get('PARITY_ACTUAL_BINARY'),
        'comparison': {'effect': 'exit-status', 'bytes': 'bash-normalized-combined-output',
                       'exit': 'expected-exit', 'snapshot': 'snapshot-normalized-output'}.get(mode),
    }
    data = (json.dumps(row, ensure_ascii=True) + '\n').encode()
    descriptor = os.open(os.environ['PARITY_EVENTS'], os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    try:
        if os.write(descriptor, data) != len(data):
            raise OSError('incomplete receipt append')
    finally:
        os.close(descriptor)


if __name__ == '__main__':
    main()
