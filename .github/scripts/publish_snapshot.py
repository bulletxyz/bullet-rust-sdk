#!/usr/bin/env python3
"""Validate and verify direct public-package snapshot publication."""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import sys
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Mapping, Sequence
from pathlib import Path

SEMVER_RE = re.compile(
    r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$"
)
STABLE_SEMVER_RE = re.compile(
    r"^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$"
)
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
BOT_LOGIN = "bullet-release[bot]"
PACKAGES = {
    "interface": "bullet-exchange-interface",
    "sdk": "bullet-rust-sdk",
}
REPOSITORIES = {
    "interface": "bulletxyz/bullet-exchange-interface",
    "sdk": "bulletxyz/bullet-rust-sdk",
}
NPM_PACKAGE = "@bulletxyz/sdk-wasm"
SLSA_V1 = "https://slsa.dev/provenance/v1"
GITHUB_BUILD = "https://slsa-framework.github.io/github-actions-buildtypes/workflow/v1"
USER_AGENT = "bullet-direct-publication/1"


class PublishError(RuntimeError):
    pass


def require_mapping(value: object, subject: str) -> Mapping[str, object]:
    if not isinstance(value, Mapping):
        raise PublishError(f"{subject} must be an object.")
    return value


def require_string(value: object, subject: str) -> str:
    if not isinstance(value, str) or not value:
        raise PublishError(f"{subject} must be a non-empty string.")
    return value


def checked_output(args: Sequence[str]) -> str:
    completed = subprocess.run(list(args), check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()[-1000:]
        raise PublishError(f"Command {' '.join(args)} failed: {detail}")
    return completed.stdout.strip()


def append_output(path: str, **values: str) -> None:
    with open(path, "a", encoding="utf-8") as output:
        output.writelines(f"{name}={value}\n" for name, value in values.items())


def commit_trailers(commit: str) -> dict[str, str]:
    raw = checked_output(
        ["git", "show", "-s", "--format=%(trailers:only,unfold=true)", commit]
    )
    trailers: dict[str, str] = {}
    for line in raw.splitlines():
        if ":" not in line:
            raise PublishError("Snapshot commit has a malformed trailer.")
        name, value = (item.strip() for item in line.split(":", 1))
        if name in trailers:
            raise PublishError(f"Snapshot commit repeats trailer {name}.")
        trailers[name] = value
    return trailers


def validate_trigger(args: argparse.Namespace) -> None:
    if args.kind not in PACKAGES:
        raise PublishError(f"Unsupported package kind {args.kind!r}.")
    if args.repository != REPOSITORIES[args.kind]:
        raise PublishError(
            f"Publication repository must be {REPOSITORIES[args.kind]}, "
            f"got {args.repository!r}."
        )
    if args.actor != BOT_LOGIN:
        raise PublishError(
            f"Publication must be triggered by {BOT_LOGIN}, got {args.actor!r}."
        )
    if args.ref != "refs/heads/main":
        raise PublishError(
            f"Publication ref must be refs/heads/main, got {args.ref!r}."
        )
    event = require_mapping(
        json.loads(Path(args.event).read_text(encoding="utf-8")), "GitHub event"
    )
    event_name = os.environ.get("GITHUB_EVENT_NAME", "")
    commit = args.sha.lower()
    if SHA_RE.fullmatch(commit) is None:
        raise PublishError("Publication commit must be a full lowercase SHA.")

    if event_name == "push":
        before = require_string(event.get("before"), "push before SHA").lower()
        if (
            event.get("after") != commit
            or event.get("ref") != "refs/heads/main"
            or event.get("forced") is not False
            or require_mapping(event.get("head_commit"), "push head commit").get("id")
            != commit
        ):
            raise PublishError("Push is not the exact non-force main snapshot update.")
        commits = event.get("commits")
        if not isinstance(commits, list) or len(commits) != 1:
            raise PublishError("Snapshot push must contain exactly one commit.")
        parents = checked_output(["git", "show", "-s", "--format=%P", commit]).split()
        if parents != [before]:
            raise PublishError(
                "Snapshot commit is not the one direct child of push before."
            )
    elif event_name == "workflow_dispatch":
        inputs = require_mapping(event.get("inputs"), "workflow dispatch inputs")
        if (
            inputs.get("kind") != args.kind
            or inputs.get("commit") != commit
            or inputs.get("version") != args.version_input
        ):
            raise PublishError("Recovery dispatch inputs are not exact.")
    else:
        raise PublishError(f"Unsupported publication event {event_name!r}.")

    head = checked_output(["git", "rev-parse", "refs/remotes/origin/main"]).lower()
    if head != commit or checked_output(["git", "rev-parse", "HEAD"]).lower() != commit:
        raise PublishError(
            "Publication commit is not the exact current public main HEAD."
        )

    trailers = commit_trailers(commit)
    package = PACKAGES[args.kind]
    version = trailers.get("Public-Version", "")
    source = trailers.get("Octopus-Source", "")
    if trailers.get("Public-Package") != package:
        raise PublishError("Snapshot package trailer does not match this workflow.")
    if STABLE_SEMVER_RE.fullmatch(version) is None:
        raise PublishError(
            "Snapshot version trailer is not exact stable semantic versioning."
        )
    if SHA_RE.fullmatch(source) is None:
        raise PublishError("Snapshot Octopus source trailer is not a full SHA.")
    interface_version = trailers.get("Interface-Version", "")
    interface_commit = trailers.get("Interface-Commit", "")
    if args.kind == "sdk" and STABLE_SEMVER_RE.fullmatch(interface_version) is None:
        raise PublishError("SDK snapshot has no exact interface version trailer.")
    if args.kind == "sdk" and SHA_RE.fullmatch(interface_commit) is None:
        raise PublishError("SDK snapshot has no exact interface commit trailer.")
    if args.kind == "interface" and interface_version:
        raise PublishError(
            "Interface snapshot unexpectedly declares an interface dependency."
        )
    if args.kind == "interface" and interface_commit:
        raise PublishError(
            "Interface snapshot unexpectedly declares an interface commit."
        )
    if event_name == "workflow_dispatch" and version != args.version_input:
        raise PublishError("Recovery version differs from the snapshot trailer.")
    append_output(
        args.output,
        commit=commit,
        version=version,
        interface_version=interface_version,
        interface_commit=interface_commit,
    )


def read_url(url: str, *, limit: int) -> bytes | None:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            raw = response.read(limit + 1)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise PublishError(f"Registry returned HTTP {error.code} for {url}.") from error
    except (OSError, urllib.error.URLError) as error:
        raise PublishError(f"Unable to read registry URL {url}: {error}") from error
    if len(raw) > limit:
        raise PublishError(f"Registry response exceeded {limit} bytes.")
    return raw


def crate_index_url(crate: str) -> str:
    normalized = crate.lower()
    if len(normalized) == 1:
        path = f"1/{normalized}"
    elif len(normalized) == 2:
        path = f"2/{normalized}"
    elif len(normalized) == 3:
        path = f"3/{normalized[0]}/{normalized}"
    else:
        path = f"{normalized[:2]}/{normalized[2:4]}/{normalized}"
    return f"https://index.crates.io/{path}"


def crate_version_exists(crate: str, version: str) -> bool:
    raw = read_url(crate_index_url(crate), limit=10 * 1024 * 1024)
    if raw is None:
        return False
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise PublishError(f"Sparse index entry for {crate} is not UTF-8.") from error

    matches = 0
    for line_number, line in enumerate(lines, start=1):
        try:
            entry = require_mapping(
                json.loads(line), f"Sparse index entry for {crate} line {line_number}"
            )
        except json.JSONDecodeError as error:
            raise PublishError(
                f"Sparse index entry for {crate} line {line_number} is malformed."
            ) from error
        if entry.get("vers") == version:
            matches += 1
    if matches > 1:
        raise PublishError(
            f"Sparse index contains duplicate {crate} {version} entries."
        )
    return matches == 1


def crate_revision(crate: str, version: str) -> str | None:
    if not crate_version_exists(crate, version):
        return None
    encoded_crate = urllib.parse.quote(crate, safe="")
    encoded_file = urllib.parse.quote(f"{crate}-{version}.crate", safe="")
    raw = read_url(
        f"https://static.crates.io/crates/{encoded_crate}/{encoded_file}",
        limit=100 * 1024 * 1024,
    )
    if raw is None:
        raise PublishError(
            f"Sparse index contains {crate} {version}, but its crate archive is absent."
        )
    import io

    try:
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as archive:
            matches = [
                member
                for member in archive.getmembers()
                if Path(member.name).name == ".cargo_vcs_info.json"
            ]
            if len(matches) != 1 or not matches[0].isfile():
                raise PublishError("Published crate has no unique Cargo VCS metadata.")
            stream = archive.extractfile(matches[0])
            if stream is None:
                raise PublishError("Published crate VCS metadata is unreadable.")
            metadata = require_mapping(json.load(stream), "Cargo VCS metadata")
    except (tarfile.TarError, json.JSONDecodeError) as error:
        raise PublishError(f"Published crate archive is malformed: {error}") from error
    git = require_mapping(metadata.get("git"), "Cargo VCS git metadata")
    revision = require_string(git.get("sha1"), "Cargo VCS revision").lower()
    dirty = git.get("dirty", False)
    if SHA_RE.fullmatch(revision) is None or not isinstance(dirty, bool):
        raise PublishError("Published crate does not have full-SHA Cargo VCS metadata.")
    return revision


def crate_status(args: argparse.Namespace) -> None:
    if (
        SEMVER_RE.fullmatch(args.version) is None
        or SHA_RE.fullmatch(args.commit) is None
    ):
        raise PublishError("Crate status requires exact version and commit values.")
    revision = crate_revision(args.crate, args.version)
    if revision is None:
        append_output(args.output, exists="false")
        return
    if revision != args.commit:
        raise PublishError(
            f"{args.crate} {args.version} belongs to {revision}, expected {args.commit}."
        )
    append_output(args.output, exists="true")


def npm_integrity_hex(integrity: str) -> str:
    if not integrity.startswith("sha512-"):
        raise PublishError("npm integrity is not sha512 SRI.")
    try:
        digest = base64.b64decode(integrity[7:], validate=True)
    except ValueError as error:
        raise PublishError("npm integrity is not valid base64.") from error
    if len(digest) != 64:
        raise PublishError("npm integrity has the wrong digest length.")
    return digest.hex()


def verify_npm_provenance(
    metadata: Mapping[str, object], *, version: str, repository: str, commit: str
) -> tuple[str, str]:
    dist = require_mapping(metadata.get("dist"), "npm dist metadata")
    integrity = require_string(dist.get("integrity"), "npm integrity")
    shasum = require_string(dist.get("shasum"), "npm shasum")
    attestations = require_mapping(
        dist.get("attestations"), "npm attestations metadata"
    )
    provenance = require_mapping(attestations.get("provenance"), "npm provenance")
    if provenance.get("predicateType") != SLSA_V1:
        raise PublishError("npm package is missing SLSA v1 provenance.")
    url = require_string(attestations.get("url"), "npm attestations URL")
    parsed = urllib.parse.urlsplit(url)
    expected_path = f"/-/npm/v1/attestations/{NPM_PACKAGE}@{version}"
    if (
        parsed.scheme != "https"
        or parsed.hostname != "registry.npmjs.org"
        or urllib.parse.unquote(parsed.path) != expected_path
        or parsed.query
        or parsed.fragment
    ):
        raise PublishError("npm attestations URL is not the exact registry endpoint.")
    raw = read_url(url, limit=10 * 1024 * 1024)
    if raw is None:
        raise PublishError("npm attestations disappeared.")
    document = require_mapping(json.loads(raw), "npm attestations response")
    entries = document.get("attestations")
    matches = (
        [
            item
            for item in entries
            if isinstance(item, Mapping) and item.get("predicateType") == SLSA_V1
        ]
        if isinstance(entries, list)
        else []
    )
    if len(matches) != 1:
        raise PublishError("npm package does not have one SLSA v1 attestation.")
    bundle = require_mapping(matches[0].get("bundle"), "npm Sigstore bundle")
    envelope = require_mapping(bundle.get("dsseEnvelope"), "npm DSSE envelope")
    payload = require_string(envelope.get("payload"), "npm provenance payload")
    statement = require_mapping(json.loads(base64.b64decode(payload)), "npm statement")
    subject = {
        "name": f"pkg:npm/{urllib.parse.quote(NPM_PACKAGE, safe='/')}@{version}",
        "digest": {"sha512": npm_integrity_hex(integrity)},
    }
    predicate = require_mapping(statement.get("predicate"), "npm predicate")
    build = require_mapping(predicate.get("buildDefinition"), "npm build definition")
    external = require_mapping(
        build.get("externalParameters"), "npm external parameters"
    )
    workflow = require_mapping(external.get("workflow"), "npm workflow identity")
    dependencies = build.get("resolvedDependencies")
    expected_ref = "refs/heads/main"
    if (
        statement.get("_type") != "https://in-toto.io/Statement/v1"
        or statement.get("predicateType") != SLSA_V1
        or statement.get("subject") != [subject]
        or build.get("buildType") != GITHUB_BUILD
        or workflow
        != {
            "repository": f"https://github.com/{repository}",
            "path": ".github/workflows/npm-publish.yml",
            "ref": expected_ref,
        }
        or dependencies
        != [
            {
                "uri": f"git+https://github.com/{repository}@{expected_ref}",
                "digest": {"gitCommit": commit},
            }
        ]
    ):
        raise PublishError("npm provenance is not bound to this main commit/workflow.")
    return integrity, shasum


def npm_status(args: argparse.Namespace) -> None:
    encoded_package = urllib.parse.quote(NPM_PACKAGE, safe="")
    encoded_version = urllib.parse.quote(args.version, safe="")
    raw = read_url(
        f"https://registry.npmjs.org/{encoded_package}/{encoded_version}",
        limit=10 * 1024 * 1024,
    )
    if raw is None:
        append_output(args.output, exists="false")
        return
    metadata = require_mapping(json.loads(raw), "npm package metadata")
    integrity, shasum = verify_npm_provenance(
        metadata,
        version=args.version,
        repository=args.repository,
        commit=args.commit,
    )
    if args.integrity and args.integrity != integrity:
        raise PublishError("Published npm integrity differs from the packed snapshot.")
    if args.shasum and args.shasum != shasum:
        raise PublishError("Published npm shasum differs from the packed snapshot.")
    append_output(args.output, exists="true", integrity=integrity, shasum=shasum)


def retry_status(args: argparse.Namespace) -> None:
    for attempt in range(1, args.attempts + 1):
        try:
            if args.artifact == "crate":
                revision = crate_revision(args.crate, args.version)
                if revision is None:
                    raise PublishError("crate has not propagated")
                if revision != args.commit:
                    raise PublishError(f"crate belongs to {revision}")
            else:
                encoded_package = urllib.parse.quote(NPM_PACKAGE, safe="")
                encoded_version = urllib.parse.quote(args.version, safe="")
                raw = read_url(
                    f"https://registry.npmjs.org/{encoded_package}/{encoded_version}",
                    limit=10 * 1024 * 1024,
                )
                if raw is None:
                    raise PublishError("npm package has not propagated")
                metadata = require_mapping(json.loads(raw), "npm package metadata")
                integrity, shasum = verify_npm_provenance(
                    metadata,
                    version=args.version,
                    repository=args.repository,
                    commit=args.commit,
                )
                if args.integrity and integrity != args.integrity:
                    raise PublishError("npm integrity differs from packed snapshot")
                if args.shasum and shasum != args.shasum:
                    raise PublishError("npm shasum differs from packed snapshot")
            return
        except PublishError:
            if attempt == args.attempts:
                raise
            time.sleep(args.interval)


def wait_crate(args: argparse.Namespace) -> None:
    if STABLE_SEMVER_RE.fullmatch(args.version) is None:
        raise PublishError("Crate wait requires an exact stable version.")
    if args.commit and SHA_RE.fullmatch(args.commit) is None:
        raise PublishError("Crate wait commit must be a full lowercase SHA.")
    for attempt in range(1, args.attempts + 1):
        try:
            revision = crate_revision(args.crate, args.version)
        except PublishError:
            if attempt == args.attempts:
                raise
        else:
            if revision is not None:
                if args.commit and revision != args.commit:
                    raise PublishError(
                        f"{args.crate} {args.version} belongs to {revision}, "
                        f"expected interface mirror {args.commit}."
                    )
                return
        if attempt == args.attempts:
            break
        time.sleep(args.interval)
    raise PublishError(f"Timed out waiting for {args.crate} {args.version}.")


def gh_json(args: Sequence[str]) -> Mapping[str, object]:
    return require_mapping(
        json.loads(checked_output(["gh", "api", *args])), "GitHub API"
    )


def finalize_release(args: argparse.Namespace) -> None:
    if (
        SEMVER_RE.fullmatch(args.version) is None
        or SHA_RE.fullmatch(args.commit) is None
    ):
        raise PublishError(
            "Release finalization requires exact version and commit values."
        )
    tag = f"v{args.version}"
    ref_path = f"repos/{args.repository}/git/ref/tags/{tag}"
    try:
        ref = gh_json([ref_path])
    except PublishError as error:
        if "HTTP 404" not in str(error):
            raise
        ref = gh_json(
            [
                "--method",
                "POST",
                f"repos/{args.repository}/git/refs",
                "-f",
                f"ref=refs/tags/{tag}",
                "-f",
                f"sha={args.commit}",
            ]
        )
    target = require_mapping(ref.get("object"), "Git tag target")
    if target.get("type") != "commit" or target.get("sha") != args.commit:
        raise PublishError(f"Existing {tag} does not point directly to {args.commit}.")

    release_path = f"repos/{args.repository}/releases/tags/{tag}"
    try:
        release = gh_json([release_path])
    except PublishError as error:
        if "HTTP 404" not in str(error):
            raise
        prerelease = "true" if "-" in args.version else "false"
        release = gh_json(
            [
                "--method",
                "POST",
                f"repos/{args.repository}/releases",
                "-f",
                f"tag_name={tag}",
                "-f",
                f"target_commitish={args.commit}",
                "-f",
                f"name={tag}",
                "-f",
                "body=Published directly from the approved Octopus snapshot.",
                "-F",
                "draft=false",
                "-F",
                f"prerelease={prerelease}",
            ]
        )
    author = require_mapping(release.get("author"), "GitHub release author")
    if (
        release.get("tag_name") != tag
        or release.get("draft") is not False
        or author.get("login") != BOT_LOGIN
    ):
        raise PublishError(f"Existing GitHub release {tag} is not the trusted release.")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    commands = result.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("validate-trigger")
    validate.add_argument("--kind", required=True)
    validate.add_argument("--event", required=True)
    validate.add_argument("--sha", required=True)
    validate.add_argument("--actor", required=True)
    validate.add_argument("--ref", required=True)
    validate.add_argument("--repository", required=True)
    validate.add_argument("--version-input", default="")
    validate.add_argument("--output", required=True)
    validate.set_defaults(handler=validate_trigger)

    crate = commands.add_parser("crate-status")
    crate.add_argument("--crate", required=True)
    crate.add_argument("--version", required=True)
    crate.add_argument("--commit", required=True)
    crate.add_argument("--output", required=True)
    crate.set_defaults(handler=crate_status)

    npm = commands.add_parser("npm-status")
    npm.add_argument("--version", required=True)
    npm.add_argument("--commit", required=True)
    npm.add_argument("--repository", required=True)
    npm.add_argument("--integrity", default="")
    npm.add_argument("--shasum", default="")
    npm.add_argument("--output", required=True)
    npm.set_defaults(handler=npm_status)

    retry = commands.add_parser("retry-status")
    retry.add_argument("--artifact", choices=("crate", "npm"), required=True)
    retry.add_argument("--crate", default="")
    retry.add_argument("--version", required=True)
    retry.add_argument("--commit", required=True)
    retry.add_argument("--repository", default="")
    retry.add_argument("--integrity", default="")
    retry.add_argument("--shasum", default="")
    retry.add_argument("--attempts", type=int, default=30)
    retry.add_argument("--interval", type=float, default=10)
    retry.set_defaults(handler=retry_status)

    wait = commands.add_parser("wait-crate")
    wait.add_argument("--crate", required=True)
    wait.add_argument("--version", required=True)
    wait.add_argument("--commit", default="")
    wait.add_argument("--attempts", type=int, default=60)
    wait.add_argument("--interval", type=float, default=10)
    wait.set_defaults(handler=wait_crate)

    finalize = commands.add_parser("finalize-release")
    finalize.add_argument("--version", required=True)
    finalize.add_argument("--commit", required=True)
    finalize.add_argument("--repository", required=True)
    finalize.set_defaults(handler=finalize_release)
    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        args.handler(args)
    except (PublishError, json.JSONDecodeError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
