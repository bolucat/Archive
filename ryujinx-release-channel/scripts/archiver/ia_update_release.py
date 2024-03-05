#!/usr/bin/env python3

import hashlib
import json
import os
from typing import Dict, Optional
import requests
import internetarchive


def get_env_variable(key: str, default_value: Optional[str] = None) -> str:
    if key in os.environ:
        return os.environ[key]
    elif default_value is not None:
        return default_value
    else:
        print(f"ERROR: Missing required env variable {key}")
        exit(1)


def download_file(
    url: str, path: str, headers: Optional[Dict[str, str]] = None
) -> bytes:
    with requests.get(url, headers=headers, stream=True) as r:
        r.raise_for_status()
        with open(path, "wb") as f:
            file_hash = hashlib.sha256()
            for chunk in r.iter_content(chunk_size=8192):
                file_hash.update(chunk)
                f.write(chunk)
    return file_hash.hexdigest()


GITHUB_EVENT_NAME = get_env_variable("GITHUB_EVENT_NAME")
GITHUB_EVENT_PATH = get_env_variable("GITHUB_EVENT_PATH")

IA_S3_ACCESS_KEY = get_env_variable("IA_S3_ACCESS_KEY")
IA_S3_SECRET_KEY = get_env_variable("IA_S3_SECRET_KEY")

RELEASE_INPUT_KEY = "release"

GITHUB_API_LATEST_RELEASE_ENDPOINT = "releases/latest"
GITHUB_API_TAG_RELEASE_ENDPOINT = "releases/tags"

with open(GITHUB_EVENT_PATH, "r") as f:
    github_event = json.load(f)

if GITHUB_EVENT_NAME == "release" and github_event["action"] == "published":
    release_object = github_event["release"]
elif GITHUB_EVENT_NAME == "workflow_dispatch":
    inputs_object = github_event["inputs"]

    if RELEASE_INPUT_KEY not in inputs_object:
        print(
            "ERROR: {GITHUB_EVENT_NAME} require the {RELEASE_INPUT_KEY} key to be present"
        )
        exit(1)

    release_input = inputs_object[RELEASE_INPUT_KEY]
    target_repository_url = github_event["repository"]["url"]

    # TODO: pass token when present
    if release_input == 'latest':
        response = requests.get(f"{target_repository_url}/{GITHUB_API_LATEST_RELEASE_ENDPOINT}")
    else:
        response = requests.get(f"{target_repository_url}/{GITHUB_API_TAG_RELEASE_ENDPOINT}/{release_input}")

    if response.status_code != 200:
        print(f"ERROR: GitHub API returned {response.status_code}")
        exit(1)

    release_object = response.json()
else:
    print(f"ERROR: Unsupported event received ({GITHUB_EVENT_NAME})")
    exit(1)

download_path = "./"

files = {}

# First we grab all assets
for asset_object in release_object["assets"]:
    asset_name = asset_object["name"]
    target_file_path = os.path.join(download_path, asset_name)
    browser_download_url = asset_object["browser_download_url"]

    # TODO: Release should expose hashes to ensure integrity.
    print(f"Downloading {asset_name} ({browser_download_url})")
    download_file(browser_download_url, target_file_path)

    files[asset_name] = target_file_path

version = release_object["name"]
identifier = f"ryujinx-{version}"

metadata = dict(
    title=f"Ryujinx {version}",
    mediatype="software",
    description="This is an automatic archived version of Ryujinx.\nFor more informations about this release please check out the official Changelog https://github.com/Ryujinx/Ryujinx/wiki/Changelog.",
    date=release_object["created_at"],
)

# TODO: Check API response status.
response = internetarchive.upload(
    identifier,
    files,
    metadata,
    delete=True,
    verify=True,
    verbose=True,
    validate_identifier=True,
    access_key=IA_S3_ACCESS_KEY,
    secret_key=IA_S3_SECRET_KEY,
)

print(f"Version {version} archived at https://archive.org/details/{identifier}")
