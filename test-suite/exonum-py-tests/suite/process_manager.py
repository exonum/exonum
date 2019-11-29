"""Tools for managing subprocesses."""
from enum import Enum, auto as enum_auto
import subprocess
from typing import NamedTuple, Optional
from threading import Thread
import os
import signal


class ProcessExitResult(Enum):
    """Result of the process termination."""

    # Process exited succesfully.
    Ok = enum_auto()
    # Process did not exit succesfully and was killed.
    Killed = enum_auto()


class ProcessOutput(NamedTuple):
    """Outputs collected during process execution."""

    exit_result: ProcessExitResult
    exit_code: int
    stdout: str
    stderr: str


class ProcessManager:
    """ProcessManager is an entity capable of running a process
    in a separate thread, joining it and collecting outputs."""

    def __init__(self, command: str):
        self._thread_handle = Thread(target=self._start_subprocess)
        self._command = command
        self._process: Optional[subprocess.Popen] = None
        self._killed = False
        self._output: Optional[ProcessOutput] = None

    def _start_subprocess(self) -> None:
        # We specify "shell=True" to be able to safely kill the process if we'll have to.
        # With this argument process will start in separate shell, not related to the shell
        # in which script is executed.
        self._process = subprocess.Popen(
            self._command, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, preexec_fn=os.setsid
        )

        exit_code = self._process.wait()
        stdout, stderr = map(lambda x: str(x, "utf-8"), self._process.communicate())

        exit_result = ProcessExitResult.Ok if not self._killed else ProcessExitResult.Killed

        self._output = ProcessOutput(exit_result, exit_code, stdout, stderr)

    def _kill_process(self) -> None:
        assert self._process is not None

        self._killed = True
        os.killpg(os.getpgid(self._process.pid), signal.SIGKILL)

    def run_sync(self) -> ProcessOutput:
        """Runs the process in the current thread, blocking until it exits."""
        self._start_subprocess()
        assert self._output is not None

        return self._output

    def start(self) -> None:
        """Launches the shell command in the separate thread."""
        self._thread_handle.setDaemon(True)
        self._thread_handle.start()

    def join_process(self, timeout: float, kill_on_timeout: bool = True) -> ProcessOutput:
        """Tries to wait until process is terminated, kills it otherwise."""
        self._thread_handle.join(timeout=timeout)

        if self._thread_handle.is_alive() and kill_on_timeout:
            # Process didn't stop, kill it.
            self._kill_process()
            # After process is killed, we can wait thread to stop finally.
            self._thread_handle.join()

        assert self._output is not None

        return self._output
