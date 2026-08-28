#!/usr/bin/env python3
"""Compute the next semver version for a workspace crate.

Usage: bump-version.py --current <version> --op <op> [--preid <id>] [--lenient]

Prints the new version to stdout and nothing else. On error, prints a
message to stderr and exits 1.
"""
import argparse
import re
import sys

VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)(?:-([a-z]+)\.(\d+))?$")

OPS = [
    "major", "minor", "patch",
    "premajor", "preminor", "prepatch",
    "prerelease", "release",
]


class InapplicableOp(Exception):
    """Raised for the two ops that don't apply to the current version."""


def parse_version(v):
    m = VERSION_RE.match(v)
    if not m:
        raise ValueError(f"'{v}' is not a valid MAJOR.MINOR.PATCH[-id.n] version")
    major, minor, patch, pre_id, pre_n = m.groups()
    pre = (pre_id, int(pre_n)) if pre_id is not None else None
    return int(major), int(minor), int(patch), pre


def format_version(major, minor, patch, pre):
    v = f"{major}.{minor}.{patch}"
    return f"{v}-{pre[0]}.{pre[1]}" if pre else v


def compare(a, b):
    """Return -1/0/1 comparing two parsed versions by semver precedence."""
    if a[:3] != b[:3]:
        return -1 if a[:3] < b[:3] else 1
    a_pre, b_pre = a[3], b[3]
    if a_pre is None and b_pre is None:
        return 0
    if a_pre is None:
        return 1  # no pre-release outranks the same M.m.p with a pre-release
    if b_pre is None:
        return -1
    a_parts, b_parts = [a_pre[0], str(a_pre[1])], [b_pre[0], str(b_pre[1])]
    for ap, bp in zip(a_parts, b_parts):
        if ap == bp:
            continue
        a_num, b_num = ap.isdigit(), bp.isdigit()
        if a_num and b_num:
            return -1 if int(ap) < int(bp) else 1
        if a_num != b_num:
            return -1 if a_num else 1  # numeric identifiers rank lower
        return -1 if ap < bp else 1
    return (len(a_parts) > len(b_parts)) - (len(a_parts) < len(b_parts))


def bump(parsed, op, preid):
    major, minor, patch, pre = parsed

    if op == "major":
        return major + 1, 0, 0, None
    if op == "minor":
        return major, minor + 1, 0, None
    if op == "patch":
        if pre is not None:
            current = format_version(*parsed)
            raise InapplicableOp(
                f"current version {current} is a pre-release; "
                "use `release` to promote it or `prerelease` to bump it"
            )
        return major, minor, patch + 1, None
    if op == "premajor":
        return major + 1, 0, 0, (preid, 1)
    if op == "preminor":
        return major, minor + 1, 0, (preid, 1)
    if op == "prepatch":
        return major, minor, patch + 1, (preid, 1)
    if op == "prerelease":
        if pre is not None:
            return (major, minor, patch, (preid, pre[1] + 1) if pre[0] == preid else (preid, 1))
        return major, minor, patch + 1, (preid, 1)
    if op == "release":
        if pre is None:
            current = format_version(*parsed)
            raise InapplicableOp(f"current version {current} is not a pre-release; nothing to promote")
        return major, minor, patch, None
    raise ValueError(f"unknown op '{op}'")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--current", required=True)
    parser.add_argument("--op", required=True, choices=OPS)
    parser.add_argument("--preid", default="rc")
    parser.add_argument("--lenient", action="store_true")
    args = parser.parse_args()

    try:
        current = parse_version(args.current)
    except ValueError as e:
        print(e, file=sys.stderr)
        sys.exit(1)

    try:
        new = bump(current, args.op, args.preid)
    except InapplicableOp as e:
        if args.lenient:
            print(args.current)
            sys.exit(0)
        print(e, file=sys.stderr)
        sys.exit(1)

    if compare(new, current) <= 0:
        print(
            f"computed {format_version(*new)} is not greater than current {args.current}",
            file=sys.stderr,
        )
        sys.exit(1)

    print(format_version(*new))


if __name__ == "__main__":
    main()
