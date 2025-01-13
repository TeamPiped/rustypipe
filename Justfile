test:
    # cargo test --features=rss
    cargo nextest run --workspace --features=rss --no-fail-fast --retries 1 -- --skip 'cookie_auth::'

unittest:
    cargo nextest run --features=rss --no-fail-fast --lib

testyt:
    cargo nextest run --features=rss --no-fail-fast --retries 1 --test youtube -- --skip 'cookie_auth::'

testyt-cookie:
    cargo nextest run --features=rss --no-fail-fast --retries 1 --test youtube

testyt-localized:
    YT_LANG=th cargo nextest run --features=rss --no-fail-fast --retries 1 --test youtube -- \
        --skip 'cookie_auth::' --skip 'search_suggestion' --skip 'isrc_search_languages'

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

        if YT_LANG="$YT_LANG" cargo nextest run --no-fail-fast --retries 1 --test-threads 4 --test youtube -- \
            --skip 'cookie_auth::' --skip 'search_suggestion' --skip 'isrc_search_languages' --skip 'resolve_'; then
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
    CHANGELOG="CHANGELOG.md"

    if [ "$CRATE" = "rustypipe" ]; then
        INCLUDES="--exclude-path 'notes/**' --exclude-path 'cli/**' --exclude-path 'downloader/**'"
    else
        if [ ! -d "$CRATE" ]; then
            echo "$CRATE does not exist."; exit 1
        fi
        INCLUDES="--include-path README.md --include-path LICENSE --include-path Cargo.toml --include-path '$CRATE/**'"
        CHANGELOG="$CRATE/$CHANGELOG"
        CRATE="rustypipe-$CRATE" # Add crate name prefix
    fi

    VERSION=$(cargo pkgid --package "$CRATE" | tr '#@' '\n' | tail -n 1)
    TAG="${CRATE}/v${VERSION}"
    echo "Releasing $TAG:"

    if git rev-parse "$TAG" >/dev/null 2>&1; then echo "version tag $TAG already exists"; exit 1; fi

    CLIFF_ARGS="--tag '${TAG}' --tag-pattern '${CRATE}/v*' --unreleased $INCLUDES"
    echo "git-cliff $CLIFF_ARGS"
    if [ -f "$CHANGELOG" ]; then
        eval "git-cliff $CLIFF_ARGS --prepend '$CHANGELOG'"
    else
        eval "git-cliff $CLIFF_ARGS --output '$CHANGELOG'"
    fi

    editor "$CHANGELOG"

    git add .
    git commit -m "chore(release): release $CRATE v$VERSION"

    awk 'BEGIN{RS="(^|\n)## [^\n]+\n*"} NR==2 { print }' "$CHANGELOG" | git tag -as -F - --cleanup whitespace "$TAG"

    echo "🚀 Run 'git push origin $TAG' to publish"
