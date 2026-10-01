# Sourced by the Swift scripts. Keeps an existing DEVELOPER_DIR.
# Otherwise selects the newest installed Xcode whose Swift tools version is at least 6.0.
# The macOS 14 GitHub runner defaults to Xcode 15.4 (Swift 5.10); Swift 6 is a side-by-side install.

if [ -z "${DEVELOPER_DIR:-}" ]; then
    swift_best_dir=
    swift_best_rank=0
    for swift_app in /Applications/Xcode.app /Applications/Xcode_*.app; do
        swift_dev="$swift_app/Contents/Developer"
        swift_bin="$swift_dev/Toolchains/XcodeDefault.xctoolchain/usr/bin/swift"
        if [ ! -x "$swift_bin" ]; then
            continue
        fi
        swift_line=$(DEVELOPER_DIR="$swift_dev" "$swift_bin" --version 2>/dev/null | awk 'NR == 1 { print; exit }' || true)
        swift_ver=$(printf '%s\n' "$swift_line" | sed -n 's/.*Swift version \([0-9][0-9.]*\).*/\1/p')
        swift_major=${swift_ver%%.*}
        case $swift_major in
            "" | *[!0-9]*) continue ;;
        esac
        if [ "$swift_major" -lt 6 ]; then
            continue
        fi
        swift_rest=${swift_ver#*.}
        if [ "$swift_rest" = "$swift_ver" ]; then
            swift_minor=0
            swift_patch=0
        else
            swift_minor=${swift_rest%%.*}
            case $swift_rest in
                *.*)
                    swift_patch=${swift_rest#*.}
                    swift_patch=${swift_patch%%.*}
                    ;;
                *) swift_patch=0 ;;
            esac
        fi
        case $swift_minor in
            "" | *[!0-9]*) continue ;;
        esac
        case $swift_patch in
            "" | *[!0-9]*) continue ;;
        esac
        swift_rank=$((swift_major * 1000000 + swift_minor * 1000 + swift_patch))
        if [ "$swift_rank" -gt "$swift_best_rank" ]; then
            swift_best_dir=$swift_dev
            swift_best_rank=$swift_rank
        fi
    done
    if [ -z "$swift_best_dir" ]; then
        echo "Swift tools 6.0 or newer is required" >&2
        exit 1
    fi
    DEVELOPER_DIR=$swift_best_dir
    export DEVELOPER_DIR
    printf 'DEVELOPER_DIR=%s\n' "$DEVELOPER_DIR" >&2
    unset swift_best_dir swift_best_rank swift_app swift_dev swift_bin swift_line swift_ver \
        swift_major swift_rest swift_minor swift_patch swift_rank
fi
