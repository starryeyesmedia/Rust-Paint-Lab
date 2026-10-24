#!/usr/bin/env python3
"""Preserve normal test output and expose failures via GitHub check annotations."""
from collections import deque
import os
import subprocess
import sys

process = subprocess.Popen(['cargo', 'test', '--workspace', '--locked'],
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                           text=True, encoding='utf-8', errors='replace')
tail = deque(maxlen=100)
for line in process.stdout:
    print(line, end='', flush=True)
    tail.append(line)
result = process.wait()
if result and os.environ.get('GITHUB_ACTIONS') == 'true':
    message = ''.join(tail)[-12000:]
    message = message.replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
    print('::error title=Workspace test failure::' + message)
sys.exit(result)
