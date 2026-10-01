#!/bin/sh
set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ] || [ -z "$1" ]; then
    echo "usage: swift/release.sh <version> [--publish]" >&2
    exit 1
fi

version=$1
publish=0
if [ "$#" -eq 2 ]; then
    if [ "$2" != "--publish" ]; then
        echo "usage: swift/release.sh <version> [--publish]" >&2
        exit 1
    fi
    publish=1
fi

major=${version%%.*}
rest=${version#*.}
minor=${rest%%.*}
patch=${rest#*.}
if [ "$major.$minor.$patch" != "$version" ] || [ -z "$major" ] || [ -z "$minor" ] || [ -z "$patch" ]; then
    echo "version must be MAJOR.MINOR.PATCH" >&2
    exit 1
fi
case $major$minor$patch in
    *[!0-9]*)
        echo "version must be MAJOR.MINOR.PATCH" >&2
        exit 1
        ;;
esac

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DIST="$ROOT/target/swift-package"
ZIP="$ROOT/target/AvdkitFFI.xcframework.zip"
ASSET="AvdkitFFI.xcframework.zip"
BRANCH="release/swift"
index=
download=
work=

cleanup() {
    if [ -n "${index:-}" ]; then
        rm -f "$index"
    fi
    if [ -n "${download:-}" ]; then
        rm -rf "$download"
    fi
    if [ -n "${work:-}" ]; then
        rm -rf "$work"
    fi
}
trap cleanup EXIT

# shellcheck source=xcode.sh
. "$ROOT/swift/xcode.sh"

cd "$ROOT"

if [ -n "${GITHUB_REPOSITORY:-}" ]; then
    repo=$GITHUB_REPOSITORY
else
    origin=$(git remote get-url origin)
    case $origin in
        *.git) origin=${origin%.git} ;;
    esac
    case $origin in
        git@github.com:*) repo=${origin#git@github.com:} ;;
        ssh://git@github.com/*) repo=${origin#ssh://git@github.com/} ;;
        https://github.com/*) repo=${origin#https://github.com/} ;;
        *)
            echo "origin is not a GitHub repository: $origin" >&2
            exit 1
            ;;
    esac
fi

url="https://github.com/${repo}/releases/download/${version}/${ASSET}"
current=$(git symbolic-ref --quiet HEAD || true)
if [ "$current" = "refs/heads/${BRANCH}" ]; then
    echo "checked out branch is ${BRANCH}; release from another branch" >&2
    exit 1
fi

if git rev-parse --verify --quiet "refs/tags/${version}" >/dev/null; then
    echo "tag ${version} already exists locally" >&2
    exit 1
fi
remote_tag=$(git ls-remote --tags origin "refs/tags/${version}") || {
    echo "could not read tags from origin" >&2
    exit 1
}
if [ -n "$remote_tag" ]; then
    echo "tag ${version} already exists on origin" >&2
    exit 1
fi

if [ -z "${GIT_AUTHOR_NAME:-}" ]; then
    GIT_AUTHOR_NAME=$(git config --get user.name 2>/dev/null || true)
fi
if [ -z "${GIT_AUTHOR_EMAIL:-}" ]; then
    GIT_AUTHOR_EMAIL=$(git config --get user.email 2>/dev/null || true)
fi
if [ -z "$GIT_AUTHOR_NAME" ] || [ -z "$GIT_AUTHOR_EMAIL" ]; then
    echo "set GIT_AUTHOR_NAME and GIT_AUTHOR_EMAIL" >&2
    exit 1
fi
if [ -z "${GIT_COMMITTER_NAME:-}" ]; then
    GIT_COMMITTER_NAME=$GIT_AUTHOR_NAME
fi
if [ -z "${GIT_COMMITTER_EMAIL:-}" ]; then
    GIT_COMMITTER_EMAIL=$GIT_AUTHOR_EMAIL
fi
export GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL

"$ROOT/swift/build.sh"
"$ROOT/swift/package.sh" "$url"
expected=$(tr -d '[:space:]' < "$ZIP.sha256")

index=$(mktemp)
GIT_INDEX_FILE=$index git read-tree --empty
GIT_INDEX_FILE=$index git --work-tree="$DIST" add -f --all
tree=$(GIT_INDEX_FILE=$index git write-tree)

files=$(git ls-tree -r --name-only "$tree" | sort)
wanted=$(printf '%s\n' \
    "Package.swift" \
    "Sources/Avdkit/Generated.swift" \
    "Sources/Avdkit/Operation+AsyncSequence.swift")
if [ "$files" != "$wanted" ]; then
    echo "release tree contains unexpected files:" >&2
    printf '%s\n' "$files" >&2
    exit 1
fi

parent=
set +e
git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1
branch_status=$?
set -e
if [ "$branch_status" -eq 0 ]; then
    git fetch --quiet origin "${BRANCH}:refs/remotes/origin/${BRANCH}"
    parent=$(git rev-parse "refs/remotes/origin/${BRANCH}")
elif [ "$branch_status" -ne 2 ]; then
    echo "could not read ${BRANCH} from origin" >&2
    exit 1
fi

if [ -n "$parent" ]; then
    commit=$(git commit-tree "$tree" -p "$parent" -m "Swift ${version}")
else
    commit=$(git commit-tree "$tree" -m "Swift ${version}")
fi

manifest=$(git show "${commit}:Package.swift")
if ! printf '%s\n' "$manifest" | grep -F "url: \"${url}\"" >/dev/null; then
    echo "release manifest URL does not match ${url}" >&2
    exit 1
fi
if ! printf '%s\n' "$manifest" | grep -F "checksum: \"${expected}\"" >/dev/null; then
    echo "release manifest checksum does not match ${expected}" >&2
    exit 1
fi

if [ "$publish" -ne 1 ]; then
    printf 'commit %s\n' "$commit"
    echo "tag ${version} was not pushed"
    exit 0
fi

if ! command -v gh >/dev/null 2>&1; then
    echo "gh is required to publish" >&2
    exit 1
fi

git tag -a "$version" "$commit" -m "$version"
git update-ref "refs/heads/${BRANCH}" "$commit"
if ! git push origin "refs/heads/${BRANCH}:refs/heads/${BRANCH}" "refs/tags/${version}:refs/tags/${version}"; then
    echo "push failed; local tag ${version} points at ${commit}" >&2
    exit 1
fi

rust_version=$(rustc --version)
swift_line=$(swift --version | awk 'NR == 1 { print; exit }')
notes="Swift package ${version}.

Rust: ${rust_version}
Swift: ${swift_line}"

attempt=0
while [ "$attempt" -lt 6 ]; do
    if gh release view "$version" --repo "$repo" >/dev/null 2>&1; then
        break
    fi
    if gh release create "$version" \
        "$ZIP" \
        "$ZIP.sha256" \
        --repo "$repo" \
        --title "$version" \
        --target "$commit" \
        --verify-tag \
        --notes "$notes"; then
        break
    fi
    attempt=$((attempt + 1))
    sleep 5
done
if ! gh release view "$version" --repo "$repo" >/dev/null 2>&1; then
    echo "GitHub Release ${version} was not created" >&2
    exit 1
fi

download=$(mktemp -d)
attempt=0
downloaded=0
while [ "$attempt" -lt 6 ]; do
    if gh release download "$version" \
        --repo "$repo" \
        --pattern "$ASSET" \
        --dir "$download" \
        --clobber; then
        downloaded=1
        break
    fi
    attempt=$((attempt + 1))
    sleep 5
done
if [ "$downloaded" -ne 1 ]; then
    echo "could not download ${ASSET} from release ${version}" >&2
    exit 1
fi
actual=$(cd "$ROOT/swift" && swift package compute-checksum "$download/$ASSET")
if [ "$actual" != "$expected" ]; then
    echo "uploaded checksum ${actual} does not match ${expected}" >&2
    exit 1
fi

name=$(printf '%s' "${repo##*/}" | tr '[:upper:]' '[:lower:]')
work=$(mktemp -d)
mkdir -p "$work/Sources/ReleaseCheck"
cat > "$work/Package.swift" <<EOF
// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "ReleaseCheck",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(url: "https://github.com/${repo}.git", exact: "${version}"),
    ],
    targets: [
        .executableTarget(
            name: "ReleaseCheck",
            dependencies: [
                .product(name: "Avdkit", package: "${name}"),
            ]
        ),
    ]
)
EOF
cat > "$work/Sources/ReleaseCheck/main.swift" <<EOF
import Avdkit

@main
struct Main {
    static func main() {
        _ = defaultKitConfig()
    }
}
EOF

attempt=0
resolved=0
while [ "$attempt" -lt 6 ]; do
    if (cd "$work" && swift package resolve); then
        resolved=1
        break
    fi
    attempt=$((attempt + 1))
    sleep 10
done
if [ "$resolved" -ne 1 ]; then
    echo "could not resolve published tag ${version}" >&2
    exit 1
fi
(cd "$work" && swift build)

printf 'published %s %s\n' "$version" "$commit"
printf '%s\n' "$url"
