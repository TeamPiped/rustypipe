test:
    cargo test --features=rss

unittest:
    cargo test --features=rss --lib

testyt:
    cargo test --features=rss --test youtube

testyt-localized:
    YT_LANG=th cargo test --features=rss --test youtube

testintl:
    #!/usr/bin/env bash
    LANGUAGES=(
        "af" "am" "ar" "as" "az" "be" "bg" "bn" "bs" "ca" "cs" "da" "de" "el"
        "en" "en-GB" "en-IN"
        "es" "es-419" "es-US" "et" "eu" "fa" "fi" "fil" "fr" "fr-CA" "gl" "gu"
        "hi" "hr" "hu" "hy" "id" "is" "it" "iw" "ja" "ka" "kk" "km" "kn" "ko" "ky"
        "lo" "lt" "lv" "mk" "ml" "mn" "mr" "ms" "my" "ne" "nl" "no" "or" "pa" "pl"
        "pt" "pt-PT" "ro" "ru" "si" "sk" "sl" "sq" "sr" "sr-Latn" "sv" "sw" "ta"
        "te" "th" "tr" "uk" "ur" "uz" "vi" "zh-CN" "zh-HK" "zh-TW" "zu"
    )

    N_FAILED=0

    for YT_LANG in "${LANGUAGES[@]}"; do
        echo "---TESTS FOR $YT_LANG ---"

        if YT_LANG="$YT_LANG" cargo test --test youtube -- --test-threads 4 --skip resolve; then
            echo "--- $YT_LANG COMPLETED ---"
        else
            echo "--- $YT_LANG FAILED ---"
            ((N_FAILED++))
        fi
    done

    exit "$N_FAILED"

testfiles:
    cargo run -p rustypipe-codegen download-testfiles

report2yaml:
    mkdir -p rustypipe_reports/conv
    for f in rustypipe_reports/*.json; do yq '.http_request.resp_body' $f | yq -o json -P > rustypipe_reports/conv/`basename $f .json`_body.json; yq e -Pi "del(.http_request.resp_body)" $f; mv $f rustypipe_reports/conv/`basename $f .json`.yaml; done;

release crate="rustypipe":
    #!/usr/bin/env bash
    set -e

    CRATE="{{crate}}"
    INCLUDES='--include-path README.md --include-path LICENSE --include-path Cargo.toml'
    CHANGELOG="CHANGELOG.md"
    CARGO_TOML="Cargo.toml"

    if [ "$CRATE" = "rustypipe" ]; then
        INCLUDES="$INCLUDES --include-path src/** --include-path tests/** --include-path testfiles/**"
    else
        if [ ! -d "$CRATE" ]; then
            echo "$CRATE does not exist."; exit 1
        fi
        INCLUDES="$INCLUDES --include-path $CRATE/**"
        CHANGELOG="$CRATE/$CHANGELOG"
        CARGO_TOML="$CRATE/Cargo.toml"
        CRATE="rustypipe-$CRATE" # Add crate name prefix
    fi

    VERSION=$(git-cliff $INCLUDES --bumped-version | grep -Po '\d+\.\d+\.\d+$')
    echo "Releasing $VERSION:"

    # if [ -n "$(git status --porcelain)" ]; then echo "Workdir must be clean"; exit 1; fi
    if git rev-parse "${CRATE}/v${VERSION}" >/dev/null 2>&1; then echo "version tag v${VERSION} already exists"; exit 1; fi

    cargo semver -c "$CARGO_TOML" set "$VERSION"

    CLIFF_ARGS="--tag v${VERSION}"
    if [ -f "$CHANGELOG" ]; then
        CLIFF_ARGS="$CLIFF_ARGS --unreleased $INCLUDES"
        CLIFF_OUT="--prepend $CHANGELOG"
    else
        CLIFF_OUT="--output $CHANGELOG"
    fi

    echo "git-cliff $CLIFF_ARGS $CLIFF_OUT"
    eval "git-cliff $CLIFF_ARGS $CLIFF_OUT"
    git add "$CHANGELOG" "$CARGO_TOML"
    git commit -m "chore(release): release $CRATE v$VERSION"

    eval "git-cliff $CLIFF_ARGS --strip all" | git tag -a -F - --cleanup whitespace "${CRATE}/v${VERSION}"
