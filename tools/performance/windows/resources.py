# SPDX-License-Identifier: GPL-3.0-or-later
"""Windows process accounting, separate from the host's per-key stopwatch."""
import ctypes as ct
from ctypes import wintypes as wt
import subprocess
import time


class MemoryStatus(ct.Structure):
    _fields_ = [("length", wt.DWORD), ("load", wt.DWORD)] + [
        (name, ct.c_ulonglong) for name in (
            "total_physical", "available_physical", "total_pagefile",
            "available_pagefile", "total_virtual", "available_virtual", "available_extended")]


class ProcessMemory(ct.Structure):
    _fields_ = [("size", wt.DWORD), ("page_faults", wt.DWORD)] + [
        (name, ct.c_size_t) for name in (
            "peak_working_set", "working_set", "quota_peak_paged", "quota_paged",
            "quota_peak_nonpaged", "quota_nonpaged", "pagefile", "peak_pagefile", "private")]


def measure_process(command, *, cwd, env, stdout):
    kernel = ct.WinDLL("kernel32", use_last_error=True)
    psapi = ct.WinDLL("psapi", use_last_error=True)
    kernel.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
    kernel.OpenProcess.restype = wt.HANDLE
    kernel.CloseHandle.argtypes = [wt.HANDLE]
    kernel.GetProcessTimes.argtypes = [wt.HANDLE] + [ct.POINTER(wt.FILETIME)] * 4
    kernel.GlobalMemoryStatusEx.argtypes = [ct.POINTER(MemoryStatus)]
    kernel.GetActiveProcessorCount.argtypes = [wt.WORD]
    kernel.GetActiveProcessorCount.restype = wt.DWORD
    psapi.GetProcessMemoryInfo.argtypes = [wt.HANDLE, ct.POINTER(ProcessMemory), wt.DWORD]

    def system_memory():
        state = MemoryStatus()
        state.length = ct.sizeof(state)
        if not kernel.GlobalMemoryStatusEx(ct.byref(state)):
            raise ct.WinError(ct.get_last_error())
        return dict(total_bytes=state.total_physical,
                    used_bytes=state.total_physical - state.available_physical,
                    load_percent=state.load)

    before = system_memory()
    logical = kernel.GetActiveProcessorCount(0xffff)
    started = time.perf_counter()
    with subprocess.Popen(command, cwd=cwd, env=env, stdout=stdout) as process:
        handle = kernel.OpenProcess(0x0400 | 0x0010, False, process.pid)
        if not handle:
            process.kill()
            raise ct.WinError(ct.get_last_error())
        samples = []
        peak_ws = peak_private = peak_commit = 0
        try:
            while True:
                counters = ProcessMemory()
                counters.size = ct.sizeof(counters)
                if psapi.GetProcessMemoryInfo(handle, ct.byref(counters), ct.sizeof(counters)):
                    samples.append(counters.working_set)
                    peak_ws = max(peak_ws, counters.peak_working_set)
                    peak_private = max(peak_private, counters.private)
                    peak_commit = max(peak_commit, counters.peak_pagefile)
                try:
                    process.wait(timeout=0.05)
                    break
                except subprocess.TimeoutExpired:
                    pass
            elapsed = time.perf_counter() - started
            times = [wt.FILETIME() for _ in range(4)]
            if not kernel.GetProcessTimes(handle, *(ct.byref(t) for t in times)):
                raise ct.WinError(ct.get_last_error())
            seconds = lambda t: ((t.dwHighDateTime << 32) | t.dwLowDateTime) / 10_000_000
            cpu = seconds(times[2]) + seconds(times[3])
            if process.returncode:
                raise subprocess.CalledProcessError(process.returncode, command)
        finally:
            kernel.CloseHandle(handle)
    if not samples:
        raise RuntimeError("No process memory samples collected")
    return dict(wall_seconds=elapsed, cpu_seconds=cpu,
                cpu_percent_one_core=cpu / elapsed * 100,
                cpu_percent_machine=cpu / elapsed / logical * 100,
                logical_processors=logical, peak_working_set_bytes=peak_ws,
                sampled_mean_working_set_bytes=sum(samples) / len(samples),
                sampled_peak_private_bytes=peak_private, peak_commit_bytes=peak_commit,
                peak_working_set_percent_physical=peak_ws / before["total_bytes"] * 100,
                memory_sample_count=len(samples), poll_interval_seconds=0.05,
                system_memory_before=before, system_memory_after=system_memory(),
                scope="Whole benchmark process: startup, dictionary/session construction, warmups, "
                      "timed keys, verification JSON output. Not per-key CPU or whole IME application. "
                      "Working set includes shared resident pages; private bytes are committed memory.")
