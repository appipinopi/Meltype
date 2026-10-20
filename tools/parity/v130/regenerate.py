#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Regenerate v1.3.0 golden responses from its unmodified, pinned Core."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[3]
COMMIT = "6310c44b728d5ad88284853f9ef37ea224993b18"
DATA = ROOT / "rust/core/tests/data"

def run(args):
    return subprocess.run(args, cwd=ROOT, check=True, capture_output=True).stdout

def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))

def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")

def main():
    output = ROOT / "dist/v130-regenerate"
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="reference-", dir=output) as temporary:
        workspace = Path(temporary).resolve()
        archive = run(["git", "archive", "--format=zip", COMMIT, "src/Meltype.Core", "dictionaries"])
        with zipfile.ZipFile(io.BytesIO(archive)) as package:
            for member in package.infolist():
                target = (workspace / member.filename).resolve()
                if not target.is_relative_to(workspace) or target == workspace:
                    raise ValueError("archive path outside isolated reference checkout")
            package.extractall(workspace)
        project = workspace / "src/Meltype.Core/Meltype.Core.csproj"
        binary = workspace / "bin"
        run(["dotnet", "build", str(ROOT / "tools/parity/v130/Reference.csproj"),
             f"-p:CoreProject={project}", "-o", str(binary), "--nologo"])
        reference = ["dotnet", str(binary / "Meltype.Core.Tests.dll")]
        values = read(DATA / "upstream-v130-values.json")
        requests = workspace / "values.json"
        write(requests, [case["request"] for case in values["cases"]])
        replies = [json.loads(line) for line in run(reference+[str(requests)]).decode("utf-8-sig").splitlines()]
        if len(replies) != len(values["cases"]):
            raise ValueError("missing scalar reference responses")
        values["cases"] = replies
        for name in ["Composition/Calculator.cs", "Composition/NumberCandidates.cs",
                     "Composition/CompositionController.cs", "Composition/Predictor.cs", "Composition/GitHubIssueTitles.cs", "Input/KeyShortcut.cs"]:
            values["source_sha256"][name] = hashlib.sha256((project.parent/name).read_bytes()).hexdigest()
        session_documents = {}
        for filename in ["upstream-v130-sessions.json", "upstream-v130-input-edges.json"]:
            sessions = read(DATA / filename)
            requests = workspace / filename
            write(requests, [{key:value for key,value in case.items() if key!="expected"} for case in sessions["cases"]])
            replies = [json.loads(line) for line in run(reference+["--sessions",str(requests),str(workspace/(filename+"-profiles"))]).decode("utf-8-sig").splitlines()]
            if len(replies) != len(sessions["cases"]):
                raise ValueError("missing key reference responses")
            for case,reply in zip(sessions["cases"],replies):
                if case["name"]!=reply["name"]:
                    raise ValueError("misordered reference response")
                case["expected"]=reply["expected"]
            session_documents[filename] = sessions
        defaults = json.loads(run(reference+["--defaults"]).decode("utf-8-sig"))
        settings = read(ROOT/"rust/core/tests/data/upstream-v130-settings.json")
        requests = workspace / "settings.json"
        write(requests, [{key:value for key,value in case.items() if key!="expected"} for case in settings["settings"]])
        replies = [json.loads(line) for line in run(reference+["--settings",str(requests),str(workspace/"settings-files")]).decode("utf-8-sig").splitlines()]
        if len(replies) != len(settings["settings"]):
            raise ValueError("missing settings reference responses")
        for case,reply in zip(settings["settings"],replies):
            if case["name"] != reply["name"]: raise ValueError("misordered settings reference")
            case["expected"] = reply["expected"]
        # Lossless shallow deltas avoid repeating complete default settings 120 times.
        base_settings = settings["settings"][0]["expected"][0]["settings"]
        settings["baseSettings"] = base_settings
        for case in settings["settings"]:
            for step in case["expected"]:
                snapshot = step.pop("settings")
                if not base_settings.keys() <= snapshot.keys():
                    raise ValueError("settings snapshot removed base keys")
                delta = {key:value for key,value in snapshot.items() if key not in base_settings or value != base_settings[key]}
                if {**base_settings, **delta} != snapshot:
                    raise ValueError("lossy settings fixture")
                step["settingsDelta"] = delta
        # Publish only after all runs completed successfully.
        write(DATA/"upstream-v130-settings.json", settings)
        write(DATA/"upstream-v130-defaults.json", {"upstream":COMMIT,"defaults":defaults})
        write(DATA/"upstream-v130-values.json",values)
        for filename, document in session_documents.items():
            write(DATA/filename, document)
        sequences = sum(len(document["cases"]) for document in session_documents.values())
        print(f"Regenerated {len(values['cases'])} scalar cases, {sequences} key sequences, and {len(settings['settings'])} settings cases from {COMMIT}")

if __name__ == "__main__":
    main()
