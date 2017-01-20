#!/usr/bin/env python
import sys
import os
import signal
from subprocess import call


def write_stdout(s):
   sys.stdout.write(s)
   sys.stdout.flush()
def write_stderr(s):
   sys.stderr.write(s)
   sys.stderr.flush()
def main():
   while 1:
       write_stdout('READY\n')
       line = sys.stdin.readline()
       write_stderr('This line kills supervisor: ' + line);
       headers = dict([ x.split(':') for x in line.split() ])
       data = sys.stdin.read(int(headers['len'])) # read the event payload
       write_stderr('Body of supervisor request: ' + data);
       body = dict([ x.split(':') for x in data.split() ])
       if body['groupname'] == 'cryptocurrency_profiler_generate' and int(body['expected']) == 1:
           try:
                   call(["supervisorctl", '-c', os.path.dirname(os.path.realpath(sys.argv[0])) + '/etc/supervisord.conf', "shutdown"])
           except Exception as e:
                   write_stderr('Could not kill supervisor: ' + e.strerror + '\n')
       write_stdout('RESULT 2\nOK')
if __name__ == '__main__':
   main()
   import sys