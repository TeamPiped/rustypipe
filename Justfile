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

testfiles:
    cargo run -p rustypipe-codegen -- -d . download-testfiles

report2yaml:
    mkdir -p rustypipe_reports/conv
    for f in rustypipe_reports/*.json; do yq '.http_request.resp_body' $f | yq -o json -P > rustypipe_reports/conv/`basename $f .json`_body.json; yq e -Pi $f; mv $f rustypipe_reports/conv/`basename $f .json`.yaml; done;
