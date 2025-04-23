use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use ress::tokens::{Keyword, Punct, Token};
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
    pub async fn extract(http: &Client, reporter: Option<&dyn Reporter>) -> Result<Self, Error> {
        let js_url = get_player_js_url(http).await?;
        let player_js = get_response(http, &js_url).await?;
        tracing::debug!("downloaded player.js from {}", js_url);

        let res = Self::extract_fns(&js_url, &player_js);

        if let Err(e) = &res {
            if let Some(reporter) = reporter {
                let report = Report {
                    info: RustyPipeInfo::new(None, None),
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
            return Err(DeobfError::Other("nsig fn returned an exception".into()));
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
    let name = get_sig_fn_name(player_js)?;
    let code = extract_js_fn(player_js, &name)?;
    let js_fn = format!("{}{}", code, caller_function(DEOBF_SIG_FUNC_NAME, &name));

    tracing::trace!("sig_fn: {js_fn}");
    verify_fn(&js_fn, DEOBF_SIG_FUNC_NAME)?;
    tracing::debug!("successfully extracted sig fn `{name}`");

    Ok(js_fn)
}

fn get_nsig_fn_names(player_js: &str) -> impl Iterator<Item = String> + '_ {
    static FUNCTION_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
        // ( ==="index.m3u8" OR "index.m3u8"=== ) .. delete .. y=functionName[array_num](z)
        Regex::new(r#"(?:(?:===(?:[\w$]+\[\d+\]|"index\.m3u8"))|(?:(?:[\w$]+\[\d+\]|"index\.m3u8")===)).+\bdelete\b.+\b[a-zA-Z]=([\w$]{2,})(?:\[(\d+)\])?\([a-zA-Z0-9]\)"#)
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

fn extract_js_fn(js: &str, name: &str) -> Result<String, DeobfError> {
    let function_base_re = Regex::new(&format!(r#"{}\s*=\s*function\("#, regex::escape(name)))
        .map_err(|e| DeobfError::Other(format!("parsing regex for {name}: {e}").into()))?;
    let offset = function_base_re
        .find(js)
        .ok_or(DeobfError::Extraction("could not find function base"))?
        .start();

    let scan = ress::Scanner::new(&js[offset..]);
    let mut state = 0;

    #[derive(Default, Clone, PartialEq, Eq)]
    struct Level {
        brace: isize,
        paren: isize,
        bracket: isize,
    }

    let mut level = Level::default();
    let mut start = 0usize;
    let mut end = 0usize;

    let mut period_before = false;
    let mut function_before = false;
    let mut idents: HashMap<String, bool> = HashMap::new();
    // Set if the current statement is a variable/function param definition
    // First value is the brace level, second is true if we are on the right hand side of an assignment
    let mut var_def_stmt: Option<(Level, bool)> = None;

    let global_objects = [
        "globalThis",
        "NaN",
        "undefined",
        "Infinity",
        "Object",
        "Function",
        "Boolean",
        "Symbol",
        "Error",
        "Number",
        "BigInt",
        "Math",
        "Date",
        "String",
        "RegExp",
        "Array",
        "Map",
        "Set",
        "eval",
        "isFinite",
        "isNaN",
        "parseFloat",
        "parseInt",
        "decodeURI",
        "decodeURIComponent",
        "encodeURI",
        "encodeURIComponent",
        "escape",
        "unescape",
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
                if token.matches_punct(Punct::Equal) {
                    state = 2;
                } else {
                    state = 0;
                }
            }
            2 => {
                match &token {
                    Token::Punct(punct) => {
                        let var_def_this_lvl = || {
                            var_def_stmt
                                .as_ref()
                                .map(|(x, _)| x == &level)
                                .unwrap_or_default()
                        };

                        match punct {
                            Punct::OpenBrace => {
                                level.brace += 1;
                            }
                            Punct::CloseBrace => {
                                if var_def_this_lvl() {
                                    var_def_stmt = None;
                                }
                                level.brace -= 1;

                                if level.brace == 0 {
                                    end = it.span.end;
                                    state = 3;
                                    break;
                                }
                            }
                            Punct::OpenParen => {
                                level.paren += 1;
                            }
                            Punct::CloseParen => {
                                if var_def_this_lvl() {
                                    var_def_stmt = None;
                                }
                                level.paren -= 1;
                            }
                            Punct::OpenBracket => {
                                level.bracket += 1;
                            }
                            Punct::CloseBracket => {
                                if var_def_this_lvl() {
                                    var_def_stmt = None;
                                }
                                level.bracket -= 1;
                            }
                            Punct::SemiColon => {
                                if var_def_this_lvl() {
                                    var_def_stmt = None;
                                }
                            }
                            Punct::Comma => {
                                if let Some((lvl, rhs)) = &mut var_def_stmt {
                                    if lvl == &level {
                                        *rhs = false;
                                    }
                                }
                            }
                            Punct::Equal => {
                                if let Some((lvl, rhs)) = &mut var_def_stmt {
                                    if lvl == &level {
                                        *rhs = true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Token::Keyword(kw) => match kw {
                        Keyword::Var(_) | Keyword::Let(_) | Keyword::Const(_) => {
                            var_def_stmt = Some((level.clone(), false));
                        }
                        Keyword::Function(_) => {
                            let mut l = level.clone();
                            l.paren += 1;
                            var_def_stmt = Some((l, false));
                        }
                        _ => {}
                    },
                    Token::Ident(id) => {
                        // Ignore object attributes
                        if !period_before && !global_objects.contains(&id.as_ref()) {
                            // If we are on the left hand side of a variable definition statement
                            // or after "function", mark the variable name as defined
                            if var_def_stmt
                                .as_ref()
                                .map(|(lvl, rhs)| lvl == &level && !rhs)
                                .unwrap_or_default()
                                || function_before
                            {
                                idents.insert(id.to_string(), true);
                            } else {
                                idents.entry(id.to_string()).or_default();
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => break,
        };
        period_before = token.matches_punct(Punct::Period);
        function_before = matches!(&token, Token::Keyword(Keyword::Function(_)));
    }

    if state != 3 {
        return Err(DeobfError::Extraction("javascript function"));
    }

    let fn_range = (offset + start)..(offset + end);
    let mut code = format!("var {};", &js[fn_range.clone()]);
    let rt = rquickjs::Runtime::new()?;

    for (ident, _) in idents.into_iter().filter(|(_, v)| !v) {
        let var_pattern_str = format!(r#"(^|[^\w$\.]){}\s*=[^=]"#, regex::escape(&ident));
        let re = Regex::new(&var_pattern_str)
            .map_err(|e| DeobfError::Other(format!("parsing regex for {ident}: {e}").into()))?;
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
                Punct::OpenBrace => braces.push(b'{'),
                Punct::OpenBracket => braces.push(b'['),
                Punct::OpenParen => braces.push(b'('),
                Punct::CloseBrace => close_brace(&mut braces, b'{')?,
                Punct::CloseBracket => close_brace(&mut braces, b'[')?,
                Punct::CloseParen => close_brace(&mut braces, b'(')?,
                Punct::Comma | Punct::SemiColon => {
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
        return Err(DeobfError::Other(
            "deobfuscation fn returned empty string".into(),
        ));
    }
    if res.starts_with("enhanced_except_") || res.ends_with(&testinp) {
        return Err(DeobfError::Other("nsig fn returned an exception".into()));
    }
    Ok(())
}

fn get_nsig_fn(player_js: &str) -> Result<String, DeobfError> {
    let extract_fn = |name: &str| -> Result<String, DeobfError> {
        let code = extract_js_fn(player_js, name)?;
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

    const SIG_DEOBF_FUNC: &str = r#"var qB={w8:function(a){a.reverse()},
EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},
Np:function(a,b){a.splice(0,b)}}; var Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};var deobf_sig=Rva;"#;
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
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "var Wka = function(d){let x=10/2;return /,,[/,913,/](,)}/};"
        );
    }

    #[test]
    fn t_extract_js_fn_eviljs() {
        // Evil JavaScript code containing braces within strings and regular expressions
        let base_js = "Wka = function(d){var x = [/,,/,913,/(,)}/,\"abcdef}\\\"\",];var y = 10/2/1;return x[1][y];}//some={}random-padding+;";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "var Wka = function(d){var x = [/,,/,913,/(,)}/,\"abcdef}\\\"\",];var y = 10/2/1;return x[1][y];};"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars() {
        let base_js = "let a1 = 42;foo();var b1=11;var da=77;bar();Wka = function(da){var xy=1+2+a1*b1;return xy;}";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        // order of variables is non-reproducible
        assert!(
            res == "var a1 = 42; var b1=11; var Wka = function(da){var xy=1+2+a1*b1;return xy;};"
                || res == "var b1=11; var a1 = 42; var Wka = function(da){var xy=1+2+a1*b1;return xy;};",
            "got {res}"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars2() {
        let base_js = "{let a1 = {v1:1,v2:2}}foo();Wka = function(d){var x=1+2+a1.v1;return x;}";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "var a1 = {v1:1,v2:2}; var Wka = function(d){var x=1+2+a1.v1;return x;};"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars3() {
        let base_js = "Wka = function(d){var x=1+2+a1[0];return x;};let a1=[1,2,3]";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "var a1=[1,2,3]; var Wka = function(d){var x=1+2+a1[0];return x;};"
        );
    }

    #[test]
    fn t_extract_js_fn_outside_vars4() {
        let base_js = "let a0=123456;let a1=function(a){return a};let Wka = function(d){var x=1+2+a1();return x;}";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "var a1=function(a){return a}; var Wka = function(d){var x=1+2+a1();return x;};"
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
    #[tokio::test]
    #[traced_test]
    async fn sig_tests() {
        let cases = [
            ("6ed0d907", "AOq0QJ8wRAIgXmPlOPSBkkUs1bYFYlJCfe29xx8j7v1pDL2QwbdV96sCIEzpWqMGkFR20CFOg51Tp-7vj_EMu-m37KtXJoOySqa0"),
            ("3bb1f723", "MyOSJXtKI3m-uME_jv7-pT12gOFC02RFkGoqWpzE0Cs69VdbwQ0LDp1v7j8xx92efCJlYFYb1sUkkBSPOlPmXgIARw8JQ0qOAOAA"),
            ("2f1832d2", "0QJ8wRAIgXmPlOPSBkkUs1bYFYlJCfe29xxAj7v1pDL0QwbdV96sCIEzpWqMGkFR20CFOg51Tp-7vj_EMu-m37KtXJ2OySqa0q"),
            ("643afba4", "AAOAOq0QJ8wRAIgXmPlOPSBkkUs1bYFYlJCfe29xx8j7vgpDL0QwbdV06sCIEzpWqMGkFR20CFOS21Tp-7vj_EMu-m37KtXJoOy1"),
            ("363db69b", "0aqSyOoJXtK73m-uME_jv7-pT15gOFC02RFkGMqWpz2ICs6EVdbwQ0LDp1v7j8xx92efCJlYFYb1sUkkBSPOlPmXgIARw8JQ0qOAOAA"),
            ("6450230e", "qax0aqSyOoJXtK73m-uME_jv7-pT152OFC02RFkGMqWpzEICs69VdbwQ0LDp1v7j8gx92efCJlYFYb1sUkkBSPOlPmXgIARw8JQ0qOAOAA"),
        ];

        for (js_hash, exp_sig) in cases {
            let span = tracing::span!(tracing::Level::ERROR, "sig_test", js_hash);
            let _enter = span.enter();

            let (js_url, js_path) = player_js_file(js_hash).await;
            let player_js = std::fs::read_to_string(js_path).unwrap();
            let deobf_data = DeobfData::extract_fns(&js_url, &player_js).unwrap();
            let deobf = Deobfuscator::new(&deobf_data).unwrap();

            let deobf_sig = deobf.deobfuscate_sig("2aq0aqSyOoJXtK73m-uME_jv7-pT15gOFC02RFkGMqWpzEICs69VdbwQ0LDp1v7j8xx92efCJlYFYb1sUkkBSPOlPmXgIARw8JQ0qOAOAA").unwrap();
            assert_eq!(deobf_sig, exp_sig, "[{js_hash}]");
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn nsig_tests() {
        let cases = [
            ("7862ca1f", "X_LCxVDjAavgE5t", "yxJ1dM6iz5ogUg"),
            ("9216d1f7", "SLp9F5bwjAdhE9F-", "gWnb9IK2DJ8Q1w"),
            ("f8cb7a3b", "oBo2h5euWy6osrUt", "ivXHpm7qJjJN"),
            ("2dfe380c", "oBo2h5euWy6osrUt", "3DIBbn3qdQ"),
            ("f1ca6900", "cu3wyu6LQn2hse", "jvxetvmlI9AN9Q"),
            ("8040e515", "wvOFaY-yjgDuIEg5", "HkfBFDHmgw4rsw"),
            ("e06dea74", "AiuodmaDDYw8d3y4bf", "ankd8eza2T6Qmw"),
            ("5dd88d1d", "kSxKFLeqzv_ZyHSAt", "n8gS8oRlHOxPFA"),
            ("324f67b9", "xdftNy7dh9QGnhW", "22qLGxrmX8F1rA"),
            ("4c3f79c5", "TDCstCG66tEAO5pR9o", "dbxNtZ14c-yWyw"),
            ("c81bbb4a", "gre3EcLurNY2vqp94", "Z9DfGxWP115WTg"),
            ("1f7d5369", "batNX7sYqIJdkJ", "IhOkL_zxbkOZBw"),
            ("009f1d77", "5dwFHw8aFWQUQtffRq", "audescmLUzI3jw"),
            ("dc0c6770", "5EHDMgYLV6HPGk_Mu-kk", "n9lUJLHbxUI0GQ"),
            ("113ca41c", "cgYl-tlYkhjT7A", "hI7BBr2zUgcmMg"),
            ("c57c113c", "M92UUMHa8PdvPd3wyM", "3hPqLJsiNZx7yA"),
            ("5a3b6271", "B2j7f_UPT4rfje85Lu_e", "m5DmNymaGQ5RdQ"),
            ("7a062b77", "NRcE3y3mVtm_cV-W", "VbsCYUATvqlt5w"),
            ("dac945fd", "o8BkRxXhuYsBCWi6RplPdP", "3Lx32v_hmzTm6A"),
            ("6f20102c", "lE8DhoDmKqnmJJ", "pJTTX6XyJP2BYw"),
            ("cfa9e7cb", "aCi3iElgd2kq0bxVbQ", "QX1y8jGb2IbZ0w"),
            ("8c7583ff", "1wWCVpRR96eAmMI87L", "KSkWAVv1ZQxC3A"),
            ("b7910ca8", "_hXMCwMt9qE310D", "LoZMgkkofRMCZQ"),
            ("590f65a6", "1tm7-g_A9zsI8_Lay_", "xI4Vem4Put_rOg"),
            ("b22ef6e7", "b6HcntHGkvBLk_FRf", "kNPW6A7FyP2l8A"),
            ("3400486c", "lL46g3XifCKUZn1Xfw", "z767lhet6V2Skl"),
            ("20dfca59", "-fLCxedkAk4LUTK2", "O8kfRq1y1eyHGw"),
            ("b12cc44b", "keLa5R2U00sR9SQK", "N1OGyujjEwMnLw"),
            ("3bb1f723", "gK15nzVyaXE9RsMP3z", "ZFFWFLPWx9DEgQ"),
            ("2f1832d2", "YWt1qdbe8SAfkoPHW5d", "RrRjWQOJmBiP"),
            ("19d2ae9d", "YWt1qdbe8SAfkoPHW5d", "CS6dVTYzpZrAZ5TD"),
            ("e7567ecf", "Sy4aDGc0VpYRR9ew_", "5UPOT1VhoZxNLQ"),
            ("d50f54ef", "Ha7507LzRmH3Utygtj", "XFTb2HoeOE5MHg"),
            ("074a8365", "Ha7507LzRmH3Utygtj", "ufTsrE0IVYrkl8v"),
            ("643afba4", "N5uAlLqm0eg1GyHO", "dCBQOejdq5s-ww"),
            ("69f581a5", "-qIP447rVlTTwaZjY", "KNcGOksBAvwqQg"),
            ("363db69b", "eWYu5d5YeY_4LyEDc", "XJQqf-N7Xra3gg"),
            ("6450230e", "eWYu5d5YeY_4LyEDc", "VfULHmlBUoDPVMN"),
        ];

        for (js_hash, nsig_in, exp_nsig) in cases {
            let span = tracing::span!(tracing::Level::ERROR, "nsig_test", js_hash);
            let _enter = span.enter();

            let (js_url, js_path) = player_js_file(js_hash).await;
            let player_js = std::fs::read_to_string(js_path).unwrap();
            let deobf_data = DeobfData::extract_fns(&js_url, &player_js).expect(js_hash);
            let deobf = Deobfuscator::new(&deobf_data).expect(js_hash);

            let deobf_nsig = deobf.deobfuscate_nsig(nsig_in).expect(js_hash);
            assert_eq!(deobf_nsig, exp_nsig, "[{js_hash}]");
        }
    }

    #[tokio::test]
    #[traced_test]
    async fn t_update() {
        let client = Client::new();
        let deobf_data = DeobfData::extract(&client, None).await.unwrap();
        let deobf = Deobfuscator::new(&deobf_data).unwrap();

        let deobf_sig = deobf.deobfuscate_sig("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i").unwrap();
        assert!(deobf_sig.len() >= 100);
        let deobf_nsig = deobf.deobfuscate_nsig("WHbZ-Nj2TSJxder").unwrap();
        assert!(deobf_nsig.len() >= 6);
    }
}
