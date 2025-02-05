use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use ress::tokens::Token;
use rquickjs::{Context, Runtime};
use serde::{Deserialize, Serialize};

use crate::{
    error::{internal::DeobfError, Error},
    report::{Level, Report, Reporter, RustyPipeInfo},
    util,
};

pub struct Deobfuscator {
    ctx: Context,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeobfData {
    pub js_url: String,
    pub sig_fn: String,
    pub nsig_fn: String,
    pub sts: String,
}

impl DeobfData {
    /// Download and extract the latest deobfuscation data from YouTube
    ///
    /// Creates a report if the data could not be extracted
    pub async fn extract(http: Client, reporter: Option<&dyn Reporter>) -> Result<Self, Error> {
        let js_url = get_player_js_url(&http).await?;
        let player_js = get_response(&http, &js_url).await?;
        tracing::debug!("downloaded player.js from {}", js_url);

        let res = Self::extract_fns(&js_url, &player_js);

        if let Err(e) = &res {
            if let Some(reporter) = reporter {
                let report = Report {
                    info: RustyPipeInfo::new(None),
                    level: Level::ERR,
                    operation: "extract_deobf",
                    error: Some(e.to_string()),
                    msgs: vec![],
                    deobf_data: None,
                    http_request: crate::report::HTTPRequest {
                        url: &js_url,
                        method: "GET",
                        req_header: None,
                        req_body: None,
                        status: 200,
                        resp_body: player_js,
                    },
                };
                reporter.report(&report);
            }
        }
        res
    }

    pub fn extract_fns(js_url: &str, player_js: &str) -> Result<Self, Error> {
        let sig_fn = get_sig_fn(player_js)?;
        let nsig_fn = get_nsig_fn(player_js)?;
        let sts = get_sts(player_js)?;

        Ok(Self {
            js_url: js_url.to_owned(),
            sig_fn,
            nsig_fn,
            sts,
        })
    }
}

impl Deobfuscator {
    /// Instantiate a new deobfuscator with the given data
    pub fn new(data: &DeobfData) -> Result<Self, DeobfError> {
        let rt = Runtime::new()?;
        let ctx = Context::full(&rt)?;
        ctx.with(|ctx| {
            let mut opts = rquickjs::context::EvalOptions::default();
            opts.strict = false;
            ctx.eval_with_options::<(), _>(data.sig_fn.as_bytes(), opts)?;
            let mut opts = rquickjs::context::EvalOptions::default();
            opts.strict = false;
            ctx.eval_with_options::<(), _>(data.nsig_fn.as_bytes(), opts)
        })?;
        Ok(Self { ctx })
    }

    /// Deobfuscate the `s` parameter from the `signature_cipher` field
    pub fn deobfuscate_sig(&self, sig: &str) -> Result<String, DeobfError> {
        let res = self
            .ctx
            .with(|ctx| call_fn(&ctx, DEOBF_SIG_FUNC_NAME, sig))?;
        tracing::trace!("deobf sig: {sig} -> {res}");
        Ok(res)
    }

    /// Deobfuscate the `n` stream URL parameter to circumvent throttling
    pub fn deobfuscate_nsig(&self, nsig: &str) -> Result<String, DeobfError> {
        let res = self
            .ctx
            .with(|ctx| call_fn(&ctx, DEOBF_NSIG_FUNC_NAME, nsig))?;
        tracing::trace!("deobf nsig: {nsig} -> {res}");
        if res.starts_with("enhanced_except_") || res.ends_with(nsig) {
            return Err(DeobfError::Other("nsig fn returned an exception"));
        }
        Ok(res)
    }
}

const DEOBF_SIG_FUNC_NAME: &str = "deobf_sig";
const DEOBF_NSIG_FUNC_NAME: &str = "deobf_nsig";

fn get_sig_fn_name(player_js: &str) -> Result<String, DeobfError> {
    let pattern = [
        r#"\b(?P<var>[\w$]+)&&\((?P=var)=(?P<sig>[\w$]{2,})\(decodeURIComponent\((?P=var)\)\)"#,
        r#"(?P<sig>[\w$]+)\s*=\s*function\(\s*(?P<arg>[\w$]+)\s*\)\s*{\s*(?P=arg)\s*=\s*(?P=arg)\.split\(\s*""\s*\)\s*;\s*[^}]+;\s*return\s+(?P=arg)\.join\(\s*""\s*\)"#,
        r#"(?:\b|[^\w$])(?P<sig>[\w$]{2,})\s*=\s*function\(\s*a\s*\)\s*{\s*a\s*=\s*a\.split\(\s*""\s*\)(?:;[\w$]{2}\.[\w$]{2}\(a,\d+\))?"#,
        r#"\b[cs]\s*&&\s*[adf]\.set\([^,]+\s*,\s*encodeURIComponent\s*\(\s*(?P<sig>[\w$]+)\("#,
        r#"\b[a-zA-Z0-9]+\s*&&\s*[a-zA-Z0-9]+\.set\([^,]+\s*,\s*encodeURIComponent\s*\(\s*(?P<sig>[\w$]+)\("#,
        r#"\bm=(?P<sig>[\w$]{2,})\(decodeURIComponent\(h\.s\)\)"#,
    ];

    util::get_cg_from_fancy_regexes(&pattern, player_js, "sig")
        .ok_or(DeobfError::Extraction("sig fn name"))
}

fn caller_function(mapped_name: &str, fn_name: &str) -> String {
    format!("var {mapped_name}={fn_name};")
}

fn get_sig_fn(player_js: &str) -> Result<String, DeobfError> {
    let dfunc_name = get_sig_fn_name(player_js)?;

    let function_pattern_str = format!(
        r#"({}=function\([\w]+\)\{{.+?\}})"#,
        dfunc_name.replace('$', "\\$")
    );
    let function_pattern = Regex::new(&function_pattern_str)
        .map_err(|_| DeobfError::Other("could not parse sig fn pattern regex"))?;

    let deobfuscate_function = format!(
        "var {};",
        &function_pattern
            .captures(player_js)
            .ok_or(DeobfError::Extraction("sig fn"))?[1]
    );

    let helper_object_name_pattern = Regex::new(r";([\w\$]{2,3})\...\(").unwrap();
    let helper_object_name = helper_object_name_pattern
        .captures(&deobfuscate_function)
        .ok_or(DeobfError::Extraction("sig fn helper object name"))?
        .get(1)
        .unwrap()
        .as_str();

    let helper_pattern_str = format!(
        r#"(var {}=\{{.+?\}}\}};)"#,
        helper_object_name.replace('$', "\\$")
    );
    let helper_pattern = Regex::new(&helper_pattern_str)
        .map_err(|_| DeobfError::Other("could not parse helper pattern regex"))?;
    let player_js_nonl = player_js.replace('\n', "");
    let helper_object = &helper_pattern
        .captures(&player_js_nonl)
        .ok_or(DeobfError::Extraction("sig fn helper object"))?[1];

    let js_fn = helper_object.to_owned()
        + &deobfuscate_function
        + &caller_function(DEOBF_SIG_FUNC_NAME, &dfunc_name);
    tracing::trace!("sig_fn: {js_fn}");
    verify_fn(&js_fn, DEOBF_SIG_FUNC_NAME)?;
    tracing::debug!("successfully extracted sig fn `{dfunc_name}`");

    Ok(js_fn)
}

fn get_nsig_fn_names(player_js: &str) -> impl Iterator<Item = String> + '_ {
    static FUNCTION_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
        // x.get( .. y=functionName[array_num](z) .. x.set(
        Regex::new(r#"(?:[\w$]\.get\(|index\.m3u8).+[a-zA-Z]=([\w$]{2,})(?:\[(\d+)\])?\([a-zA-Z0-9]\).+[a-zA-Z0-9]\.set\("#)
            .unwrap()
    });

    FUNCTION_NAME_REGEX
        .captures_iter(player_js)
        .filter_map(|fname_match| {
            let function_name = &fname_match[1];

            match fname_match.get(2) {
                Some(array_num) => {
                    let array_num = array_num.as_str().parse::<usize>().ok()?;
                    let array_pattern_str =
                        format!(r#"var {}\s*=\s*\[(.+?)]"#, regex::escape(function_name));
                    let array_pattern = Regex::new(&array_pattern_str).ok()?;

                    let array_str = &array_pattern.captures(player_js)?[1];
                    array_str.split(',').nth(array_num).map(str::to_owned)
                }
                None => Some(function_name.to_owned()),
            }
        })
}

fn extract_js_fn(js: &str, offset: usize, name: &str) -> Result<String, DeobfError> {
    let scan = ress::Scanner::new(&js[offset..]);
    let mut state = 0;
    let mut level = 0;

    let mut start = 0;
    let mut end = 0;

    let mut period_before = false;
    let mut last_ident = None;
    let mut idents: HashMap<String, usize> = HashMap::new();

    let global_objects = [
        "NaN", "Infinity", "Object", "Function", "Boolean", "Symbol", "Error", "Number", "BigInt",
        "Math", "Date", "String", "RegExp", "Array", "Map", "Set",
    ];

    for item in scan {
        let it = item?;
        let token = it.token;
        match state {
            // Looking for fn name
            0 => {
                if token.matches_ident_str(name) {
                    state = 1;
                    start = it.span.start;
                }
            }
            // Looking for equals
            1 => {
                if token.matches_punct(ress::tokens::Punct::Equal) {
                    state = 2;
                } else {
                    state = 0;
                }
            }
            2 => {
                // Looking for begin/end braces
                if token.matches_punct(ress::tokens::Punct::OpenBrace) {
                    level += 1;
                } else if token.matches_punct(ress::tokens::Punct::CloseBrace) {
                    level -= 1;

                    if level == 0 {
                        end = it.span.end;
                        state = 3;
                        break;
                    }
                }

                // Looking for variable names
                if let Token::Ident(id) = &token {
                    if !period_before {
                        let id_str = id.to_string();
                        if !global_objects.contains(&id_str.as_str()) {
                            last_ident = Some(id.to_string());
                        }
                    }
                } else if last_ident.is_some()
                    && !token.matches_punct(ress::tokens::Punct::OpenParen)
                {
                    let n = idents.entry(last_ident.unwrap()).or_default();
                    *n += 1;
                    last_ident = None;
                } else {
                    last_ident = None;
                }
            }
            _ => break,
        };
        period_before = token.matches_punct(ress::tokens::Punct::Period);
    }

    if state != 3 {
        return Err(DeobfError::Extraction("javascript function"));
    }

    let fn_range = (offset + start)..(offset + end);
    let mut code = format!("var {};", &js[fn_range.clone()]);
    let rt = rquickjs::Runtime::new()?;

    for (ident, _) in idents.into_iter().filter(|(_, v)| *v == 1) {
        let var_pattern_str = format!(r#"(^|[^\w$]){}\s*=[^=]"#, regex::escape(&ident));
        let re = Regex::new(&var_pattern_str).unwrap();
        let found_variable = re
            .captures_iter(js)
            .filter(|cap| {
                let m = cap.get(0).unwrap();
                !fn_range.contains(&m.start()) && !fn_range.contains(&m.end())
            })
            .find_map(|cap| extract_js_var(&js[cap.get(1).unwrap().end()..]));
        if let Some(var_code) = found_variable {
            let ctx = Context::full(&rt)?;
            let var_code = format!("var {var_code};");
            if let Err(e) = ctx.with(|ctx| ctx.eval::<(), _>(var_code.as_bytes())) {
                tracing::warn!("invalid var ({e}): {var_code}");
                code = format!("var {ident}={{}}; {code}");
            } else {
                code = format!("{var_code} {code}");
            }
        }
    }
    Ok(code)
}

fn extract_js_var(js: &str) -> Option<&str> {
    let scan = ress::Scanner::new(js);
    let mut braces: Vec<u8> = Vec::new();
    let mut end = 0;

    let close_brace = |braces: &mut Vec<u8>, c: u8| -> Option<()> {
        if let Some(brace) = braces.last() {
            if *brace == c {
                braces.pop();
                Some(())
            } else {
                None
            }
        } else {
            None
        }
    };

    for item in scan {
        let it = match item {
            Ok(it) => it,
            Err(e) => {
                // If the variable definition is the last statement in a closure and followed by a }
                // the scanner thinks the code is invalid
                if e.msg == "unmatched close brace" && braces.is_empty() {
                    end = e.idx;
                    break;
                } else {
                    return None;
                }
            }
        };
        let token = it.token;

        if let Token::Punct(p) = &token {
            match p {
                ress::tokens::Punct::OpenBrace => braces.push(b'{'),
                ress::tokens::Punct::OpenBracket => braces.push(b'['),
                ress::tokens::Punct::OpenParen => braces.push(b'('),
                ress::tokens::Punct::CloseBrace => close_brace(&mut braces, b'{')?,
                ress::tokens::Punct::CloseBracket => close_brace(&mut braces, b'[')?,
                ress::tokens::Punct::CloseParen => close_brace(&mut braces, b'(')?,
                ress::tokens::Punct::Comma | ress::tokens::Punct::SemiColon => {
                    if braces.is_empty() {
                        end = it.span.start;
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    if end > 0 {
        Some(&js[0..end])
    } else if braces.is_empty() {
        Some(js)
    } else {
        None
    }
}

fn call_fn(ctx: &rquickjs::Ctx, fn_name: &str, arg: &str) -> Result<String, rquickjs::Error> {
    let f: rquickjs::Function = ctx.globals().get(fn_name)?;
    f.call((arg,))
}

/// Verify if the deobfuscation function successfully processes a random input string
fn verify_fn(js_fn: &str, fn_name: &str) -> Result<(), DeobfError> {
    let rt = Runtime::new()?;
    let ctx = Context::full(&rt)?;
    let testinp = util::generate_content_playback_nonce();
    let res = ctx.with(|ctx| {
        ctx.eval::<(), _>(js_fn)?;
        call_fn(&ctx, fn_name, &testinp)
    })?;

    if res.is_empty() {
        return Err(DeobfError::Other("deobfuscation fn returned empty string"));
    }
    if res.starts_with("enhanced_except_") || res.ends_with(&testinp) {
        return Err(DeobfError::Other("nsig fn returned an exception"));
    }
    Ok(())
}

fn get_nsig_fn(player_js: &str) -> Result<String, DeobfError> {
    let extract_fn = |name: &str| -> Result<String, DeobfError> {
        let function_base = format!("{name}=function");
        let offset = player_js
            .find(&function_base)
            .ok_or(DeobfError::Extraction("could not find function base"))?;

        let code = extract_js_fn(player_js, offset, name)?;

        let js_fn = format!("{}{}", code, caller_function(DEOBF_NSIG_FUNC_NAME, name));
        tracing::trace!("nsig_fn: {js_fn}");
        verify_fn(&js_fn, DEOBF_NSIG_FUNC_NAME)?;
        tracing::debug!("successfully extracted nsig fn `{name}`");
        Ok(js_fn)
    };

    util::find_map_or_last_err(
        get_nsig_fn_names(player_js),
        DeobfError::Extraction("nsig function name"),
        |name| {
            extract_fn(&name).map_err(|e| {
                tracing::warn!("Failed to extract nsig fn `{name}`: {e}");
                e
            })
        },
    )
}

async fn get_player_js_url(http: &Client) -> Result<String, Error> {
    let resp = http
        .get("https://www.youtube.com/iframe_api")
        .send()
        .await?
        .error_for_status()?;
    let text = resp.text().await?;

    let player_hash_pattern =
        Regex::new(r"https:\\/\\/www\.youtube\.com\\/s\\/player\\/([a-z0-9]{8})\\/").unwrap();
    let player_hash = &player_hash_pattern
        .captures(&text)
        .ok_or(DeobfError::Extraction("player hash"))?[1];

    Ok(format!(
        "https://www.youtube.com/s/player/{player_hash}/player_ias.vflset/en_US/base.js"
    ))
}

async fn get_response(http: &Client, url: &str) -> Result<String, Error> {
    let resp = http.get(url).send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

fn get_sts(player_js: &str) -> Result<String, DeobfError> {
    let sts_pattern = Regex::new("signatureTimestamp[=:](\\d+)").unwrap();

    Ok(sts_pattern
        .captures(player_js)
        .ok_or(DeobfError::Extraction("sts"))?[1]
        .to_owned())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::util::tests::TESTFILES;
    use path_macro::path;
    use rstest::{fixture, rstest};
    use tracing_test::traced_test;

    static TEST_JS: Lazy<String> = Lazy::new(|| {
        let js_path = path!(*TESTFILES / "deobf" / "dummy_player.js");
        std::fs::read_to_string(js_path).unwrap()
    });

    const SIG_DEOBF_FUNC: &str = r#"var qB={w8:function(a){a.reverse()},EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},Np:function(a,b){a.splice(0,b)}};var Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};var deobf_sig=Rva;"#;
    const NSIG_DEOBF_FUNC: &str = r#"var Vo=function(a){var b=a.split(""),c=[function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(""))},
928409064,-595856984,1403221911,653089124,-168714481,-1883008765,158931990,1346921902,361518508,1403221911,-362174697,-233641452,function(){for(var d=64,e=[];++d-e.length-32;){switch(d){case 91:d=44;continue;case 123:d=65;break;case 65:d-=18;continue;case 58:d=96;continue;case 46:d=95}e.push(String.fromCharCode(d))}return e},
b,158931990,791141857,-907319795,-1776185924,1595027902,-829736173,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(0,1,d.splice(e,1,d[0])[0])},
-1274951142,function(){for(var d=64,e=[];++d-e.length-32;){switch(d){case 91:d=44;continue;case 123:d=65;break;case 65:d-=18;continue;case 58:d=96;continue;case 46:d=95}e.push(String.fromCharCode(d))}return e},
1758743891,function(d){d.reverse()},
-830417133,"AF43j",1942017693,function(d,e){e=(e%d.length+d.length)%d.length;d.splice(e,1)},
null,-959991459,-287691724,-1365731946,b,1250397544,-1883008765,-1912322658,b,1300441121,null,-1962382380,1954679120,function(d){for(var e=d.length;e;)d.push(d.splice(--e,1)[0])},
-985125467,function(d,e){for(e=(e%d.length+d.length)%d.length;e--;)d.unshift(d.pop())},
null,497372841,-1912651541,function(d,e){d.push(e)},
function(d,e){e=(e%d.length+d.length)%d.length;d.splice(-e).reverse().forEach(function(f){d.unshift(f)})},
function(d,e){e=(e%d.length+d.length)%d.length;var f=d[0];d[0]=d[e];d[e]=f}];
c[30]=c;c[40]=c;c[46]=c;try{c[43](c[34]),c[45](c[40],c[47]),c[46](c[51],c[33]),c[16](c[47],c[36]),c[38](c[31],c[49]),c[16](c[11],c[39]),c[0](c[11]),c[35](c[0],c[30]),c[35](c[4],c[17]),c[34](c[48],c[7],c[11]()),c[35](c[4],c[23]),c[35](c[4],c[9]),c[5](c[48],c[28]),c[36](c[46],c[16]),c[4](c[41],c[1]),c[4](c[16],c[28]),c[3](c[40],c[17]),c[9](c[8],c[23]),c[45](c[30],c[4]),c[50](c[3],c[28]),c[36](c[51],c[23]),c[14](c[0],c[24]),c[14](c[35],c[1]),c[20](c[51],c[41]),c[15](c[8],c[0]),c[31](c[35]),c[29](c[26]),
c[36](c[8],c[32]),c[20](c[25],c[10]),c[2](c[22],c[8]),c[32](c[20],c[16]),c[32](c[47],c[49]),c[1](c[44],c[28]),c[39](c[16]),c[32](c[42],c[22]),c[46](c[14],c[48]),c[26](c[29],c[10]),c[46](c[9],c[3]),c[32](c[45])}catch(d){return"enhanced_except_85UBjOr-_w8_"+a}return b.join("")};var deobf_nsig=Vo;"#;

    #[fixture]
    fn deobf() -> Deobfuscator {
        Deobfuscator::new(&DeobfData {
            js_url: String::default(),
            sig_fn: SIG_DEOBF_FUNC.to_owned(),
            nsig_fn: NSIG_DEOBF_FUNC.to_owned(),
            sts: String::default(),
        })
        .unwrap()
    }

    #[test]
    fn t_get_sig_fn_name() {
        let dfunc_name = get_sig_fn_name(&TEST_JS).unwrap();
        assert_eq!(dfunc_name, "Rva");
    }

    #[test]
    fn t_get_sig_fn() {
        let dcode = get_sig_fn(&TEST_JS).unwrap();
        assert_eq!(dcode, SIG_DEOBF_FUNC);
    }

    #[rstest]
    fn t_deobfuscate_sig(deobf: Deobfuscator) {
        let dsig = deobf.deobfuscate_sig("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i").unwrap();
        assert_eq!(dsig, "AOq0QJ8wRAIgaryQHmplJ9xJSKFywyaSMHuuwZYsoMTfvRviG51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5f");
    }

    #[test]
    fn t_get_nsig_fn_names() {
        let names = get_nsig_fn_names(&TEST_JS).collect::<Vec<_>>();
        assert_eq!(names, ["Vo"]);
    }

    #[test]
    fn t_extract_js_fn() {
        let base_js = "Wka = function(d){let x=10/2;return /,,[/,913,/](,)}/}let a = 42;";
        let res = extract_js_fn(base_js, 0, "Wka").unwrap();
        assert_eq!(
            res,
            "var Wka = function(d){let x=10/2;return /,,[/,913,/](,)}/};"
        );
    }

    #[test]
    fn t_extract_js_fn_eviljs() {
        // Evil JavaScript code containing braces within strings and regular expressions
        let base_js = "Wka = function(d){var x = [/,,/,913,/(,)}/,\"abcdef}\\\"\",];var y = 10/2/1;return x[1][y];}//some={}random-padding+;";
        let res = extract_js_fn(base_js, 0, "Wka").unwrap();
        assert_eq!(
            res,
            "var Wka = function(d){var x = [/,,/,913,/(,)}/,\"abcdef}\\\"\",];var y = 10/2/1;return x[1][y];};"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars() {
        let base_js = "let a = 42;foo();var b=11;bar();Wka = function(d){var x=1+2+a*b;return x;}";
        let res = extract_js_fn(base_js, 0, "Wka").unwrap();
        // order of variables is non-reproducible
        assert!(
            res == "var a = 42; var b=11; var Wka = function(d){var x=1+2+a*b;return x;};"
                || res == "var b=11; var a = 42; var Wka = function(d){var x=1+2+a*b;return x;};",
            "got {res}"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars2() {
        let base_js = "{let a = {v1:1,v2:2}}foo();Wka = function(d){var x=1+2+a.v1;return x;}";
        let res = extract_js_fn(base_js, 0, "Wka").unwrap();
        assert_eq!(
            res,
            "var a = {v1:1,v2:2}; var Wka = function(d){var x=1+2+a.v1;return x;};"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars3() {
        let base_js = "Wka = function(d){var x=1+2+a[0];return x;};let a=[1,2,3]";
        let res = extract_js_fn(base_js, 0, "Wka").unwrap();
        assert_eq!(
            res,
            "var a=[1,2,3]; var Wka = function(d){var x=1+2+a[0];return x;};"
        );
    }

    #[test]
    fn t_get_nsig_fn() {
        let res = get_nsig_fn(&TEST_JS).unwrap();
        assert_eq!(res, NSIG_DEOBF_FUNC);
    }

    #[test]
    fn t_get_sts() {
        let res = get_sts(&TEST_JS).unwrap();
        assert_eq!(res, "19187");
    }

    #[rstest]
    fn t_deobfuscate_nsig(deobf: Deobfuscator) {
        let res = deobf.deobfuscate_nsig("BI_n4PxQ22is-KKajKUW").unwrap();
        assert_eq!(res, "nrkec0fwgTWolw");
    }

    #[tokio::test]
    async fn t_get_player_js_url() {
        let client = Client::new();
        let url = get_player_js_url(&client).await.unwrap();
        assert!(url.starts_with("https://www.youtube.com/s/player"));
        assert_eq!(url.len(), 73);
    }

    async fn player_js_file(js_hash: &str) -> (String, PathBuf) {
        let url =
            format!("https://www.youtube.com/s/player/{js_hash}/player_ias.vflset/en_US/base.js");
        let mut js_path = path!(*TESTFILES / "deobf" / "player_js");
        std::fs::create_dir_all(&js_path).unwrap();
        js_path.push(format!("{js_hash}.js"));
        if !js_path.is_file() {
            let http = reqwest::Client::new();
            let res = http
                .get(&url)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
            let content = res.text().await.unwrap();
            let js_path_tmp = js_path.with_extension("tmp");
            std::fs::write(&js_path_tmp, &content).unwrap();
            std::fs::rename(&js_path_tmp, &js_path).unwrap();
        }
        (url, js_path)
    }

    // Test cases from https://github.com/yt-dlp/yt-dlp/blob/master/test/test_youtube_signature.py

    #[rstest]
    #[case("6ed0d907", "AOq0QJ8wRAIgXmPlOPSBkkUs1bYFYlJCfe29xx8j7v1pDL2QwbdV96sCIEzpWqMGkFR20CFOg51Tp-7vj_EMu-m37KtXJoOySqa0")]
    #[case("3bb1f723", "MyOSJXtKI3m-uME_jv7-pT12gOFC02RFkGoqWpzE0Cs69VdbwQ0LDp1v7j8xx92efCJlYFYb1sUkkBSPOlPmXgIARw8JQ0qOAOAA")]
    #[case("2f1832d2", "0QJ8wRAIgXmPlOPSBkkUs1bYFYlJCfe29xxAj7v1pDL0QwbdV96sCIEzpWqMGkFR20CFOg51Tp-7vj_EMu-m37KtXJ2OySqa0q")]
    #[tokio::test]
    #[traced_test]
    async fn sig_tests(#[case] js_hash: &str, #[case] exp_sig: &str) {
        let (js_url, js_path) = player_js_file(js_hash).await;
        let player_js = std::fs::read_to_string(js_path).unwrap();
        let deobf_data = DeobfData::extract_fns(&js_url, &player_js).unwrap();
        let deobf = Deobfuscator::new(&deobf_data).unwrap();

        let deobf_sig = deobf.deobfuscate_sig("2aq0aqSyOoJXtK73m-uME_jv7-pT15gOFC02RFkGMqWpzEICs69VdbwQ0LDp1v7j8xx92efCJlYFYb1sUkkBSPOlPmXgIARw8JQ0qOAOAA").unwrap();
        assert_eq!(deobf_sig, exp_sig, "js: {js_hash}");
    }

    #[rstest]
    #[case("7862ca1f", "X_LCxVDjAavgE5t", "yxJ1dM6iz5ogUg")]
    #[case("9216d1f7", "SLp9F5bwjAdhE9F-", "gWnb9IK2DJ8Q1w")]
    #[case("f8cb7a3b", "oBo2h5euWy6osrUt", "ivXHpm7qJjJN")]
    #[case("2dfe380c", "oBo2h5euWy6osrUt", "3DIBbn3qdQ")]
    #[case("f1ca6900", "cu3wyu6LQn2hse", "jvxetvmlI9AN9Q")]
    #[case("8040e515", "wvOFaY-yjgDuIEg5", "HkfBFDHmgw4rsw")]
    #[case("e06dea74", "AiuodmaDDYw8d3y4bf", "ankd8eza2T6Qmw")]
    #[case("5dd88d1d", "kSxKFLeqzv_ZyHSAt", "n8gS8oRlHOxPFA")]
    #[case("324f67b9", "xdftNy7dh9QGnhW", "22qLGxrmX8F1rA")]
    #[case("4c3f79c5", "TDCstCG66tEAO5pR9o", "dbxNtZ14c-yWyw")]
    #[case("c81bbb4a", "gre3EcLurNY2vqp94", "Z9DfGxWP115WTg")]
    #[case("1f7d5369", "batNX7sYqIJdkJ", "IhOkL_zxbkOZBw")]
    #[case("009f1d77", "5dwFHw8aFWQUQtffRq", "audescmLUzI3jw")]
    #[case("dc0c6770", "5EHDMgYLV6HPGk_Mu-kk", "n9lUJLHbxUI0GQ")]
    #[case("113ca41c", "cgYl-tlYkhjT7A", "hI7BBr2zUgcmMg")]
    #[case("c57c113c", "M92UUMHa8PdvPd3wyM", "3hPqLJsiNZx7yA")]
    #[case("5a3b6271", "B2j7f_UPT4rfje85Lu_e", "m5DmNymaGQ5RdQ")]
    #[case("7a062b77", "NRcE3y3mVtm_cV-W", "VbsCYUATvqlt5w")]
    #[case("dac945fd", "o8BkRxXhuYsBCWi6RplPdP", "3Lx32v_hmzTm6A")]
    #[case("6f20102c", "lE8DhoDmKqnmJJ", "pJTTX6XyJP2BYw")]
    #[case("cfa9e7cb", "aCi3iElgd2kq0bxVbQ", "QX1y8jGb2IbZ0w")]
    #[case("8c7583ff", "1wWCVpRR96eAmMI87L", "KSkWAVv1ZQxC3A")]
    #[case("b7910ca8", "_hXMCwMt9qE310D", "LoZMgkkofRMCZQ")]
    #[case("590f65a6", "1tm7-g_A9zsI8_Lay_", "xI4Vem4Put_rOg")]
    #[case("b22ef6e7", "b6HcntHGkvBLk_FRf", "kNPW6A7FyP2l8A")]
    #[case("3400486c", "lL46g3XifCKUZn1Xfw", "z767lhet6V2Skl")]
    #[case("20dfca59", "-fLCxedkAk4LUTK2", "O8kfRq1y1eyHGw")]
    #[case("b12cc44b", "keLa5R2U00sR9SQK", "N1OGyujjEwMnLw")]
    #[case("3bb1f723", "gK15nzVyaXE9RsMP3z", "ZFFWFLPWx9DEgQ")]
    #[case("2f1832d2", "YWt1qdbe8SAfkoPHW5d", "RrRjWQOJmBiP")]
    #[case("19d2ae9d", "YWt1qdbe8SAfkoPHW5d", "CS6dVTYzpZrAZ5TD")]
    #[tokio::test]
    #[traced_test]
    async fn nsig_tests(#[case] js_hash: &str, #[case] nsig_in: &str, #[case] expect: &str) {
        let (js_url, js_path) = player_js_file(js_hash).await;
        let player_js = std::fs::read_to_string(js_path).unwrap();
        let deobf_data = DeobfData::extract_fns(&js_url, &player_js).unwrap();
        let deobf = Deobfuscator::new(&deobf_data).unwrap();

        let deobf_nsig = deobf.deobfuscate_nsig(nsig_in).unwrap();
        assert_eq!(deobf_nsig, expect, "js: {js_hash}");
    }

    #[tokio::test]
    #[traced_test]
    async fn t_update() {
        let client = Client::new();
        let deobf_data = DeobfData::extract(client, None).await.unwrap();
        let deobf = Deobfuscator::new(&deobf_data).unwrap();

        let deobf_sig = deobf.deobfuscate_sig("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i").unwrap();
        assert!(deobf_sig.len() >= 100);
        let deobf_nsig = deobf.deobfuscate_nsig("WHbZ-Nj2TSJxder").unwrap();
        assert!(deobf_nsig.len() >= 6);
    }
}
