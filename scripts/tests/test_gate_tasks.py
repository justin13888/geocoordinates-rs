"""Fixture tests for the gate tasks defined inline in `.mise.toml`.

Each test reads a task's `run` script straight from `.mise.toml`, fills in its
`{{ vars.* }}` references from the same file's `[vars]` table, and runs it with
bash inside a throwaway git repository holding only the fixture files, so the
script under test is the one the gate runs. Run through `mise run scripts-test`.
"""

import os
import pathlib
import re
import subprocess
import tempfile
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONFIG = tomllib.loads((ROOT / ".mise.toml").read_text(encoding="utf-8"))
MAX_LINES = int(CONFIG["vars"]["max_file_lines"])
MISSING_REF = "refs/heads/no-such-base"


def task_script(name):
    """The task's `run` script with its `{{ vars.NAME }}` references filled in."""
    script = CONFIG["tasks"][name]["run"]
    script = re.sub(
        r"\{\{\s*vars\.(\w+)\s*\}\}", lambda m: CONFIG["vars"][m.group(1)], script
    )
    if "{{" in script:
        raise AssertionError(f"{name}: unrendered template left in its script")
    return script


def git(repo, *args):
    subprocess.run(
        ["git", "-c", "commit.gpgsign=false", *args],
        cwd=repo,
        check=True,
        capture_output=True,
    )


class GateTask(unittest.TestCase):
    """A fresh git repository per test, and a runner for one task in it."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.repo = pathlib.Path(tmp.name)
        git(self.repo, "init", "-q", "-b", "master")

    def write(self, path, text):
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def commit(self):
        git(self.repo, "add", "-A")
        git(
            self.repo,
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-q",
            "-m",
            "fixture",
        )

    def run_task(self, name, base, ci=False):
        env = {k: v for k, v in os.environ.items() if k != "CI"}
        env["usage_base"] = base
        if ci:
            env["CI"] = "true"
        return subprocess.run(
            ["bash", "-c", task_script(name)],
            cwd=self.repo,
            env=env,
            capture_output=True,
            text=True,
        )

    def assertPasses(self, result):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def assertFails(self, result):
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)


class FileLengthCheck(GateTask):
    def setUp(self):
        super().setUp()
        self.write(".file-length-baseline", "# empty baseline\n")

    def test_terminated_file_over_the_limit_fails(self):
        self.write("long.txt", "x\n" * (MAX_LINES + 1))
        self.commit()
        self.assertFails(self.run_task("file-length-check", "HEAD"))

    def test_unterminated_last_line_is_counted(self):
        self.write("long.txt", "x\n" * MAX_LINES + "x")
        self.commit()
        result = self.run_task("file-length-check", "HEAD")
        self.assertFails(result)
        self.assertIn(f"long.txt: {MAX_LINES + 1} lines", result.stdout)

    def test_unterminated_file_at_the_limit_passes(self):
        self.write("long.txt", "x\n" * (MAX_LINES - 1) + "x")
        self.write("empty.txt", "")
        self.commit()
        self.assertPasses(self.run_task("file-length-check", "HEAD"))

    def test_missing_base_fails_under_ci(self):
        self.write("short.txt", "x\n")
        self.commit()
        result = self.run_task("file-length-check", MISSING_REF, ci=True)
        self.assertFails(result)
        self.assertIn(MISSING_REF, result.stderr)

    def test_missing_base_warns_and_skips_locally(self):
        self.write("short.txt", "x\n")
        self.commit()
        result = self.run_task("file-length-check", MISSING_REF)
        self.assertPasses(result)
        self.assertIn("warning", result.stderr)


class CoverageRatchet(GateTask):
    def setUp(self):
        super().setUp()
        self.write(".coverage-baseline", "80\n")
        self.commit()

    def test_present_base_compares(self):
        self.assertPasses(self.run_task("coverage-ratchet", "HEAD"))
        self.write(".coverage-baseline", "79\n")
        self.assertFails(self.run_task("coverage-ratchet", "HEAD"))

    def test_missing_base_fails_under_ci(self):
        result = self.run_task("coverage-ratchet", MISSING_REF, ci=True)
        self.assertFails(result)
        self.assertIn(MISSING_REF, result.stderr)

    def test_missing_base_warns_and_skips_locally(self):
        result = self.run_task("coverage-ratchet", MISSING_REF)
        self.assertPasses(result)
        self.assertIn("warning", result.stderr)


GRADLE_READS_CARGO = 'version = (findProperty("releaseVersion") as String?) ?: workspaceVersion\n'


class VersionCheck(GateTask):
    def setUp(self):
        super().setUp()
        self.write(
            "Cargo.toml",
            '[workspace.package]\nversion = "1.2.3"\n\n'
            "[package]\nname = \"a\"\nversion.workspace = true\n",
        )
        self.write(
            "crates/geocoordinates-ffi/Cargo.toml",
            '[package]\nname = "b"\nversion.workspace = true\n',
        )
        web = "crates/geocoordinates-ffi/web"
        self.write(f"{web}/package.json", '{"version": "1.2.3"}\n')
        self.write(
            f"{web}/package-lock.json",
            '{"version": "1.2.3", "packages": {"": {"version": "1.2.3"}}}\n',
        )
        self.write(
            "crates/geocoordinates-ffi/pyproject.toml",
            '[project]\nname = "c"\ndynamic = ["version"]\n',
        )

    def gradle(self, text):
        self.write("crates/geocoordinates-ffi/jvm/build.gradle.kts", text)

    def test_version_read_from_cargo_passes(self):
        self.gradle(GRADLE_READS_CARGO)
        self.assertPasses(self.run_task("version-check", "HEAD"))

    def test_literal_version_fails(self):
        self.gradle('version = "1.2.3"\n')
        self.assertFails(self.run_task("version-check", "HEAD"))

    def test_literal_project_version_fails(self):
        self.gradle('project.version = "1.2.3"\n')
        self.assertFails(self.run_task("version-check", "HEAD"))


if __name__ == "__main__":
    unittest.main()
