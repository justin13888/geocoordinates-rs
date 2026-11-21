"""Fixture tests for the gate tasks defined in `.mise.toml`.

Each test reads a task's `run` script straight from `.mise.toml`, fills in its
`{{ vars.* }}` references from the same file's `[vars]` table (and
`{{ config_root }}` with this checkout, for a task that calls a script under
`scripts/`), and runs it with bash inside a throwaway git repository holding
only the fixture files, so the script under test is the one the gate runs. Run through `mise run scripts-test`.
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
    """The task's `run` script with its `{{ vars.NAME }}` and `{{ config_root }}`
    references filled in, the latter with this checkout's root, so a task that
    calls a script under `scripts/` runs this checkout's copy of it."""
    script = CONFIG["tasks"][name]["run"]
    script = re.sub(
        r"\{\{\s*vars\.(\w+)\s*\}\}", lambda m: CONFIG["vars"][m.group(1)], script
    )
    script = re.sub(r"\{\{\s*config_root\s*\}\}", lambda _: str(ROOT), script)
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

    def run_task(self, name, base, ci=False, extra_env=None):
        env = {k: v for k, v in os.environ.items() if k != "CI"}
        env["usage_base"] = base
        if ci:
            env["CI"] = "true"
        env.update(extra_env or {})
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

    def test_indented_literal_version_fails(self):
        self.gradle('allprojects {\n    version = "1.2.3"\n}\n')
        self.assertFails(self.run_task("version-check", "HEAD"))

    def test_plugin_version_is_not_the_project_version(self):
        self.gradle('plugins {\n    kotlin("jvm") version "2.1.0"\n}\n' + GRADLE_READS_CARGO)
        self.assertPasses(self.run_task("version-check", "HEAD"))


# A stand-in for `cargo` on PATH: it prints the listing file the test wrote,
# but only for `cargo mutants ... --list --no-config`. Like cargo-mutants, it
# colors the listing when CARGO_TERM_COLOR=always unless `--colors never` is
# passed, with escapes inside each name, so a colored listing matches no entry.
STUB_CARGO = r"""#!/usr/bin/env python3
import os, pathlib, re, sys
args = sys.argv[1:]
if os.environ.get("STUB_EXIT"):
    sys.exit(int(os.environ["STUB_EXIT"]))
if args[:1] != ["mutants"] or "--list" not in args or "--no-config" not in args:
    sys.exit(f"stub cargo: unexpected arguments {args}")
colors = os.environ.get("CARGO_TERM_COLOR", "auto")
if "--colors" in args:
    colors = args[args.index("--colors") + 1]
for line in pathlib.Path(os.environ["STUB_LISTING"]).read_text().splitlines():
    if colors == "always":
        line = re.sub(r"^([^:]+)", "\x1b[35m\\1\x1b[0m", line)
    print(line)
"""

SOURCE = "pub fn add(a: u32, b: u32) -> u32 {\n    // sum\n    a + b\n}\n"
LISTING = "src/lib.rs:3:7: replace + with - in add\nsrc/lib.rs:3:7: replace + with * in add\n"
ENTRY = r"'src/lib\.rs:3:.*: replace \+ with - in add',"


class MutantsAnchorCheck(GateTask):
    def setUp(self):
        super().setUp()
        self.write("src/lib.rs", SOURCE)
        self.write("listing.txt", LISTING)
        bin_dir = self.repo / "bin"
        bin_dir.mkdir()
        cargo = bin_dir / "cargo"
        cargo.write_text(STUB_CARGO, encoding="utf-8")
        cargo.chmod(0o755)
        self.env = {
            "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
            "STUB_LISTING": str(self.repo / "listing.txt"),
        }

    def config(self, *lines):
        body = "".join(f"    {line}\n" for line in lines)
        self.write(".cargo/mutants.toml", f"exclude_re = [\n{body}]\n")

    def check(self, **env):
        return self.run_task(
            "mutants-anchor-check", "HEAD", extra_env={**self.env, **env}
        )

    def test_anchored_entry_naming_its_expression_passes(self):
        self.config("# argument", "# anchor: a + b", ENTRY, "'src/lib\\.rs:.*: other',")
        result = self.check()
        self.assertPasses(result)
        self.assertIn("1 anchored exclusions", result.stdout)

    def test_colored_listing_still_matches(self):
        self.config("# anchor: a + b", ENTRY)
        self.assertPasses(self.check(CARGO_TERM_COLOR="always"))

    def test_stub_colored_listing_matches_no_entry(self):
        # Without `--colors never` the stub's listing defeats the entry, so the
        # test above passes only because the task turns color off.
        listing = subprocess.run(
            [self.repo / "bin" / "cargo", "mutants", "--list", "--no-config"],
            env={**os.environ, **self.env, "CARGO_TERM_COLOR": "always"},
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        self.assertIn("\x1b[", listing)
        pattern = ENTRY.strip("',")
        self.assertFalse(any(re.search(pattern, n) for n in listing.splitlines()))

    def test_missing_anchor_fails(self):
        self.config("# argument", ENTRY)
        result = self.check()
        self.assertFails(result)
        self.assertIn("has no `# anchor:` comment", result.stderr)

    def test_anchor_not_directly_above_an_entry_fails(self):
        self.config("# anchor: a + b", "# argument", ENTRY)
        result = self.check()
        self.assertFails(result)
        self.assertIn("is not directly above an anchored entry", result.stderr)

    def test_trailing_anchor_fails(self):
        self.config("# anchor: a + b", ENTRY, "# anchor: a + b")
        result = self.check()
        self.assertFails(result)
        self.assertIn("is not directly above an anchored entry", result.stderr)

    def test_text_not_on_the_named_line_fails(self):
        self.config("# anchor: a - b", ENTRY)
        result = self.check()
        self.assertFails(result)
        self.assertIn("`a - b` is not on src/lib.rs:3", result.stderr)

    def test_line_past_the_end_of_the_file_fails(self):
        self.config("# anchor: a + b", ENTRY.replace(":3:", ":30:"))
        result = self.check()
        self.assertFails(result)
        self.assertIn("is not on src/lib.rs:30", result.stderr)

    def test_regex_matching_no_mutant_fails(self):
        self.config("# anchor: a + b", ENTRY.replace("with -", "with /"))
        result = self.check()
        self.assertFails(result)
        self.assertIn("matches no mutant", result.stderr)

    def test_non_plain_path_fails(self):
        self.config("# anchor: a + b", ENTRY.replace("lib\\.rs", "(lib|main)\\.rs"))
        result = self.check()
        self.assertFails(result)
        self.assertIn("is not a plain path", result.stderr)

    def test_entry_the_line_scanner_missed_fails(self):
        self.write(".cargo/mutants.toml", f"exclude_re = [{ENTRY}]\n")
        result = self.check()
        self.assertFails(result)
        self.assertIn("is not written on a line of its own", result.stderr)

    def test_failing_listing_fails(self):
        self.config("# anchor: a + b", ENTRY)
        result = self.check(STUB_EXIT="3")
        self.assertFails(result)
        self.assertIn("exited 3", result.stderr)


if __name__ == "__main__":
    unittest.main()
