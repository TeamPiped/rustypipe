test:
    cargo test -F all

testfiles:
    cargo run -p rustypipe-codegen -- -d . download-testfiles

report2yaml:
    mkdir -p rustypipe_reports/conv
    for f in rustypipe_reports/*.json; do yq '.http_request.resp_body' $f | yq -o json -P > rustypipe_reports/conv/`basename $f .json`_body.json; yq e -Pi $f; mv $f rustypipe_reports/conv/`basename $f .json`.yaml; done;
