report2yaml:
    yq e -Pi rustypipe_reports/*.json
    for f in rustypipe_reports/*.json; do mv $f rustypipe_reports/`basename $f .json`.yaml; done;
