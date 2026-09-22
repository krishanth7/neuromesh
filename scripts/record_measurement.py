"""Run the real localhost QUIC experiment and retain samples plus provenance."""
import datetime
import json
import os
from pathlib import Path
import platform
import subprocess
from check_interop import ROOT, source_hashes


def main():
    output = subprocess.check_output([
        'cargo', 'run', '--locked', '--release', '-p', 'neuromesh-network',
        '--example', 'measure_quic'], cwd=ROOT, timeout=300)
    measurement = json.loads(output)
    samples = sorted(measurement['rtt_us'])
    if len(samples) != 100 or measurement['p50_us'] != samples[49] or measurement['p95_us'] != samples[94]:
        raise RuntimeError('sample count or nearest-rank percentile mismatch')
    report = {
        'recorded_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'platform': platform.platform(), 'architecture': platform.machine(),
        'available_logical_cpus': len(os.sched_getaffinity(0)) if hasattr(os, 'sched_getaffinity') else os.cpu_count(),
        'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'source_sha256': source_hashes(),
        'method': 'Single process, two authenticated localhost QUIC endpoints, release build, 10 warmups, 100 sequential exchanges, nearest-rank percentiles. RTT includes transport ACK wait. One run, no CPU isolation or WAN emulation.',
        'measurement': measurement,
    }
    path = ROOT / 'docs/results/localhost-quic.json'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(measurement))


if __name__ == '__main__':
    main()
