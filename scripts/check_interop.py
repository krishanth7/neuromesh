"""Compile and verify 15 Ping encoders against shared cases and the Rust decoder.

Missing runtimes fail the requested run. --languages supports an explicit subset;
reports always identify exactly what ran. SQL aggregates the actual results.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import platform
import sqlite3
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "target/interop"


def run(cmd, **kwargs):
    return subprocess.run([str(x) for x in cmd], cwd=ROOT, capture_output=True, timeout=120, **kwargs)


def checked(cmd):
    result = run(cmd)
    if result.returncode:
        raise RuntimeError(f"{cmd}: {result.stderr.decode(errors='replace')}")
    return result


def source_hashes():
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    for folder in ["crates", "interop", "scripts"]:
        paths.extend(p for p in (ROOT / folder).rglob("*") if p.is_file() and "__pycache__" not in p.parts)
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}


def commands():
    return {
        "rust": (["cargo", "build", "--locked", "-p", "neuromesh-protocol", "--examples"], [ROOT / "target/debug/examples/nmp_ping"], ["rustc", "--version"]),
        "python": ([], [sys.executable, "interop/python/ping.py"], [sys.executable, "--version"]),
        "javascript": ([], ["node", "interop/javascript/ping.mjs"], ["node", "--version"]),
        "typescript": ([], ["node", "interop/typescript/ping.ts"], ["node", "--version"]),
        "go": (["go", "build", "-o", BUILD / "ping-go", "interop/go/ping.go"], [BUILD / "ping-go"], ["go", "version"]),
        "c": (["gcc", "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "interop/c/ping.c", "-o", BUILD / "ping-c"], [BUILD / "ping-c"], ["gcc", "--version"]),
        "cpp": (["g++", "-std=c++17", "-Wall", "-Wextra", "-Werror", "-O2", "interop/cpp/ping.cpp", "-o", BUILD / "ping-cpp"], [BUILD / "ping-cpp"], ["g++", "--version"]),
        "java": (["javac", "-Xlint:all", "-Werror", "-d", BUILD, "interop/java/Ping.java"], ["java", "-cp", BUILD, "Ping"], ["javac", "-version"]),
        "csharp": (["mcs", "-warnaserror", f"-out:{BUILD / 'Ping.exe'}", "interop/csharp/Ping.cs"], ["mono", BUILD / "Ping.exe"], ["mcs", "--version"]),
        "ruby": ([], ["ruby", "interop/ruby/ping.rb"], ["ruby", "--version"]),
        "php": ([], ["php", "interop/php/ping.php"], ["php", "--version"]),
        "perl": ([], ["perl", "interop/perl/ping.pl"], ["perl", "-e", "print $^V"]),
        "lua": ([], ["lua5.4", "interop/lua/ping.lua"], ["lua5.4", "-v"]),
        "r": ([], ["Rscript", "interop/r/ping.R"], ["Rscript", "--version"]),
        "haskell": (["ghc", "-Wall", "-Werror", "-outputdir", BUILD, "-o", BUILD / "ping-haskell", "interop/haskell/Ping.hs"], [BUILD / "ping-haskell"], ["ghc", "--version"]),
    }


def cases():
    node = bytes(range(32)).hex()
    good = [(node, "1", "0"), (node.upper(), "42", "42"), ("ff"*32, "9007199254740993", "9007199254740993"), ("00"*32, "18446744073709551615", "18446744073709551615")]
    bad = [[], [node], [node, "1"], [node, "1", "0", "extra"]]
    for value in ["", "a"*63, "a"*65, "g"*64, "+1"*32, "é"*32, node+"\n"]:
        bad.append([value, "1", "0"])
    for value in ["", "-1", "+1", "01", "1.0", "1e2", " 1", "1\n", "١", "18446744073709551616", "9"*100]:
        bad.extend([[node, value, "0"], [node, "1", value]])
    bad.append([node, "0", "0"])
    return [(list(a), True) for a in good] + [(a, False) for a in bad]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--languages", nargs="+", choices=list(commands()), default=list(commands()))
    parser.add_argument("--output", type=Path, default=ROOT / "target/interop-results.json")
    args = parser.parse_args()
    if len(args.languages) != len(set(args.languages)):
        parser.error("duplicate languages are not allowed")
    BUILD.mkdir(parents=True, exist_ok=True)
    checked(["cargo", "build", "--locked", "-p", "neuromesh-protocol", "--examples"])
    oracle = ROOT / "target/debug/examples/check_envelope"
    rows = []
    versions = {}
    for language in args.languages:
        build, cmd, version = commands()[language]
        if build:
            checked(build)
        v = checked(version)
        versions[language] = (v.stdout + v.stderr).decode().strip().splitlines()[0]
        for index, (argv, valid) in enumerate(cases()):
            result = run([*cmd, *argv])
            passed = result.returncode > 0 and result.stdout == b"" and bool(result.stderr) if not valid else False
            if valid and result.returncode == 0:
                expected = {"version": 1, "message_id": int(argv[1]), "sender": list(bytes.fromhex(argv[0])), "message": {"type": "Ping", "body": {"nonce": int(argv[2])}}}
                try:
                    passed = json.loads(result.stdout) == expected and run([oracle], input=result.stdout).returncode == 0
                except (ValueError, UnicodeError):
                    passed = False
            rows.append({"language": language, "case_id": index, "expected_valid": valid, "passed": passed})
            if not passed:
                print(f"FAIL {language} case {index}: {result.stderr.decode(errors='replace')[:200]}", file=sys.stderr)
        print(f"{language}: {sum(r['passed'] for r in rows if r['language']==language)}/{len(cases())}", flush=True)
    with sqlite3.connect(":memory:") as db:
        db.executescript((ROOT / "interop/sql/schema.sql").read_text())
        db.executemany("INSERT INTO conformance VALUES (:language, :case_id, :expected_valid, :passed)", rows)
        summary = [dict(zip(["language", "cases", "passed", "failed"], row)) for row in db.execute((ROOT / "interop/sql/summary.sql").read_text())]
    expected_summary = [{"language": lang, "cases": len(cases()), "passed": sum(r["passed"] for r in rows if r["language"] == lang), "failed": sum(not r["passed"] for r in rows if r["language"] == lang)} for lang in sorted(args.languages)]
    if summary != expected_summary:
        raise RuntimeError("SQL aggregation disagrees with observed case outcomes")
    report = {"recorded_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "platform": platform.platform(), "architecture": platform.machine(), "base_commit": checked(["git", "rev-parse", "HEAD"]).stdout.decode().strip(), "source_sha256": source_hashes(), "versions": versions, "sql_version": sqlite3.sqlite_version, "summary": summary, "cases": rows, "all_passed": all(r["passed"] for r in rows)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2)+"\n")
    print(f"Total: {sum(r['passed'] for r in rows)}/{len(rows)}; SQL summary verified")
    return 0 if report["all_passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
