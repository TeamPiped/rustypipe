use criterion::{criterion_group, criterion_main, Criterion};
use rusty_pipe::*;

const TEST_JS: &str = include_str!("../notes/base.js");

fn bench_match_to_closing_parenthesis(c: &mut Criterion) {
    c.bench_function("match_to_closing_parenthesis", |b| {
        b.iter(|| match_to_closing_parenthesis(TEST_JS, "Vo=function"))
    });
}

fn bench_sig(c: &mut Criterion) {
    c.bench_function("deobf_sig", |b| {
        b.iter(|| {
            let dcode = load_deobfuscation_code(TEST_JS).unwrap();
            deobfuscate_signature("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i", &dcode).unwrap()
        })
    });
    
    c.bench_function("deobf_nsig", |b| {
        b.iter(|| {
            let name = get_n_deobfuscation_function_name(TEST_JS).unwrap();
            let dcode = parse_n_decode_function(TEST_JS, &name).unwrap();
            deobfuscate_n_signature(&dcode, &name, "BI_n4PxQ22is-KKajKUW").unwrap()
        })
    });
}

fn bench_sig_cached(c: &mut Criterion) {
    let sig_dcode = load_deobfuscation_code(TEST_JS).unwrap();
    let nsig_name = get_n_deobfuscation_function_name(TEST_JS).unwrap();
    let nsig_dcode = parse_n_decode_function(TEST_JS, &nsig_name).unwrap();

    c.bench_function("deobf_sig_cached", |b| {
        b.iter(|| deobfuscate_signature("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i", &sig_dcode).unwrap())
    });
    
    c.bench_function("deobf_sig_cached", |b| {
        b.iter(|| deobfuscate_n_signature(&nsig_dcode, &nsig_name, "BI_n4PxQ22is-KKajKUW").unwrap())
    });
}

criterion_group!(benches, bench_match_to_closing_parenthesis, bench_sig, bench_sig_cached);
criterion_main!(benches);
