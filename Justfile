test:
    cargo test --all-features

unittest:
    cargo test --all-features --lib

testyt:
    cargo test --all-features --test youtube

testyt10:
    #!/usr/bin/env bash
    set -e
    for i in {1..10}; do \
        echo "---TEST RUN $i---"; \
        cargo test --all-features --test youtube; \
    done

testintl:
    #!/usr/bin/env bash
    set -e
    LANGUAGES=(
        "af" "am" "ar" "as" "az" "be" "bg" "bn" "bs" "ca" "cs" "da" "de" "el" "en" "en-GB" "en-IN"
        "es" "es-419" "es-US" "et" "eu" "fa" "fi" "fil" "fr" "fr-CA" "gl" "gu"
        "hi" "hr" "hu" "hy" "id" "is" "it" "iw" "ja" "ka" "kk" "km" "kn" "ko" "ky"
        "lo" "lt" "lv" "mk" "ml" "mn" "mr" "ms" "my" "ne" "nl" "no" "or" "pa" "pl"
        "pt" "pt-PT" "ro" "ru" "si" "sk" "sl" "sq" "sr" "sr-Latn" "sv" "sw" "ta"
        "te" "th" "tr" "uk" "ur" "uz" "vi" "zh-CN" "zh-HK" "zh-TW" "zu"
    )
    for YT_LANG in "${LANGUAGES[@]}"; do \
        echo "---TESTS FOR $YT_LANG ---"; \
        YT_LANG="$YT_LANG" cargo test --test youtube -- --skip music_artist --skip music_playlist --skip get_video_details --skip startpage; \
        echo "--- $YT_LANG COMPLETED ---"; \
        sleep 10; \
    done

testfiles:
    cargo run -p rustypipe-codegen -- -d . download-testfiles

report2yaml:
    mkdir -p rustypipe_reports/conv
    for f in rustypipe_reports/*.json; do yq '.http_request.resp_body' $f | yq -o json -P > rustypipe_reports/conv/`basename $f .json`_body.json; yq e -Pi $f; mv $f rustypipe_reports/conv/`basename $f .json`.yaml; done;
