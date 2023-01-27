use fancy_regex::Regex as FancyRegex;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::result::Result::Ok;

use crate::{error::DeobfError, util};

type Result<T> = core::result::Result<T, DeobfError>;

pub struct Deobfuscator {
    data: DeobfData,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeobfData {
    pub js_url: String,
    pub sig_fn: String,
    pub nsig_fn: String,
    pub sts: String,
}

impl Deobfuscator {
    pub async fn new(http: Client) -> Result<Self> {
        let js_url = get_player_js_url(&http).await?;
        let player_js = get_response(&http, &js_url).await?;

        log::debug!("downloaded player.js from {}", js_url);

        let sig_fn = get_sig_fn(&player_js)?;
        let nsig_fn = get_nsig_fn(&player_js)?;
        let sts = get_sts(&player_js)?;

        Ok(Self {
            data: DeobfData {
                js_url,
                nsig_fn,
                sig_fn,
                sts,
            },
        })
    }

    pub fn deobfuscate_sig(&self, sig: &str) -> Result<String> {
        deobfuscate_sig(sig, &self.data.sig_fn)
    }

    pub fn deobfuscate_nsig(&self, nsig: &str) -> Result<String> {
        deobfuscate_nsig(nsig, &self.data.nsig_fn)
    }

    pub fn get_sts(&self) -> String {
        self.data.sts.to_owned()
    }

    pub fn get_data(&self) -> DeobfData {
        self.data.to_owned()
    }
}

impl From<DeobfData> for Deobfuscator {
    fn from(data: DeobfData) -> Self {
        Self { data }
    }
}

const DEOBFUSCATION_FUNC_NAME: &str = "deobfuscate";

fn get_sig_fn_name(player_js: &str) -> Result<String> {
    static FUNCTION_REGEXES: Lazy<[FancyRegex; 6]> = Lazy::new(|| {
        [
        FancyRegex::new("(?:\\b|[^a-zA-Z0-9$])([a-zA-Z0-9$]{2,})\\s*=\\s*function\\(\\s*a\\s*\\)\\s*\\{\\s*a\\s*=\\s*a\\.split\\(\\s*\"\"\\s*\\)").unwrap(),
        FancyRegex::new("\\bm=([a-zA-Z0-9$]{2,})\\(decodeURIComponent\\(h\\.s\\)\\)").unwrap(),
        FancyRegex::new("\\bc&&\\(c=([a-zA-Z0-9$]{2,})\\(decodeURIComponent\\(c\\)\\)").unwrap(),
        FancyRegex::new("([\\w$]+)\\s*=\\s*function\\((\\w+)\\)\\{\\s*\\2=\\s*\\2\\.split\\(\"\"\\)\\s*;").unwrap(),
        FancyRegex::new("\\b([\\w$]{2,})\\s*=\\s*function\\((\\w+)\\)\\{\\s*\\2=\\s*\\2\\.split\\(\"\"\\)\\s*;").unwrap(),
        FancyRegex::new("\\bc\\s*&&\\s*d\\.set\\([^,]+\\s*,\\s*(:encodeURIComponent\\s*\\()([a-zA-Z0-9$]+)\\(").unwrap(),
    ]
    });

    util::get_cg_from_fancy_regexes(FUNCTION_REGEXES.iter(), player_js, 1)
        .ok_or(DeobfError::Extraction("deobf function name"))
}

fn caller_function(fn_name: &str) -> String {
    format!("var {}={};", DEOBFUSCATION_FUNC_NAME, fn_name)
}

fn get_sig_fn(player_js: &str) -> Result<String> {
    let dfunc_name = get_sig_fn_name(player_js)?;

    let function_pattern_str =
        "(".to_owned() + &dfunc_name.replace('$', "\\$") + "=function\\([a-zA-Z0-9_]+\\)\\{.+?\\})";
    let function_pattern = Regex::new(&function_pattern_str)
        .map_err(|_| DeobfError::Other("could not parse function pattern regex"))?;

    let deobfuscate_function = "var ".to_owned()
        + function_pattern
            .captures(player_js)
            .ok_or(DeobfError::Extraction("deobf function"))?
            .get(1)
            .unwrap()
            .as_str()
        + ";";

    static HELPER_OBJECT_NAME_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(";([A-Za-z0-9_\\$]{2})\\...\\(").unwrap());
    let helper_object_name = HELPER_OBJECT_NAME_REGEX
        .captures(&deobfuscate_function)
        .ok_or(DeobfError::Extraction("helper object name"))?
        .get(1)
        .unwrap()
        .as_str();

    let helper_pattern_str =
        "(var ".to_owned() + &helper_object_name.replace('$', "\\$") + "=\\{.+?\\}\\};)";
    let helper_pattern = Regex::new(&helper_pattern_str)
        .map_err(|_| DeobfError::Other("could not parse helper pattern regex"))?;
    let player_js_nonl = player_js.replace('\n', "");
    let helper_object = helper_pattern
        .captures(&player_js_nonl)
        .ok_or(DeobfError::Extraction("helper object"))?
        .get(1)
        .unwrap()
        .as_str();

    Ok(helper_object.to_owned() + &deobfuscate_function + &caller_function(&dfunc_name))
}

fn deobfuscate_sig(sig: &str, sig_fn: &str) -> Result<String> {
    let context =
        quick_js::Context::new().or(Err(DeobfError::Other("could not create QuickJS rt")))?;
    context.eval(sig_fn)?;
    let res = context.call_function(DEOBFUSCATION_FUNC_NAME, vec![sig])?;

    res.as_str().map_or(
        Err(DeobfError::Other("sig deobfuscation func returned null")),
        |res| Ok(res.to_owned()),
    )
}

fn get_nsig_fn_name(player_js: &str) -> Result<String> {
    static FUNCTION_NAME_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new("\\.get\\(\"n\"\\)\\)&&\\([a-zA-Z0-9$_]=([a-zA-Z0-9$_]+)(?:\\[(\\d+)])?\\([a-zA-Z0-9$_]\\)")
            .unwrap()
    });

    let fname_match = FUNCTION_NAME_REGEX
        .captures(player_js)
        .ok_or(DeobfError::Extraction("n_deobf function"))?;

    let function_name = fname_match.get(1).unwrap().as_str();

    if fname_match.len() == 1 {
        return Ok(function_name.to_owned());
    }

    let array_num = fname_match
        .get(2)
        .unwrap()
        .as_str()
        .parse::<usize>()
        .or(Err(DeobfError::Other("could not parse array_num")))?;
    let array_pattern_str =
        "var ".to_owned() + &regex::escape(function_name) + "\\s*=\\s*\\[(.+?)];";
    let array_pattern = Regex::new(&array_pattern_str).or(Err(DeobfError::Other(
        "could not parse helper pattern regex",
    )))?;

    let array_str = array_pattern
        .captures(player_js)
        .ok_or(DeobfError::Extraction("n_deobf array_str"))?
        .get(1)
        .unwrap()
        .as_str();

    let mut names = array_str.split(',');
    let name = names
        .nth(array_num)
        .ok_or(DeobfError::Extraction("n_deobf function name"))?;
    Ok(name.to_owned())
}

fn extract_js_fn(js: &str, name: &str) -> Result<String> {
    let scan = ress::Scanner::new(js);
    let mut state = 0;
    let mut level = 0;

    let mut start = 0;
    let mut end = 0;

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
            // Looking for begin/end braces
            2 => {
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
            }
            _ => break,
        };
    }

    if state != 3 {
        return Err(DeobfError::Extraction("javascript function"));
    }

    Ok(js[start..end].to_owned())
}

fn get_nsig_fn(player_js: &str) -> Result<String> {
    let function_name = get_nsig_fn_name(player_js)?;
    let function_base = function_name.to_owned() + "=function";
    let offset = player_js.find(&function_base).unwrap_or_default();

    extract_js_fn(&player_js[offset..], &function_name)
        .map(|s| s + ";" + &caller_function(&function_name))
}

fn deobfuscate_nsig(sig: &str, nsig_fn: &str) -> Result<String> {
    let context =
        quick_js::Context::new().or(Err(DeobfError::Other("could not create QuickJS rt")))?;
    context.eval(nsig_fn)?;
    let res = context.call_function(DEOBFUSCATION_FUNC_NAME, vec![sig])?;

    res.as_str().map_or(
        Err(DeobfError::Other("nsig deobfuscation func returned null")),
        |res| Ok(res.to_owned()),
    )
}

async fn get_player_js_url(http: &Client) -> Result<String> {
    let resp = http
        .get("https://www.youtube.com/iframe_api")
        .send()
        .await?
        .error_for_status()?;
    let text = resp.text().await?;

    static PLAYER_HASH_PATTERN: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"https:\\/\\/www\.youtube\.com\\/s\\/player\\/([a-z0-9]{8})\\/"#).unwrap()
    });
    let player_hash = PLAYER_HASH_PATTERN
        .captures(&text)
        .ok_or(DeobfError::Extraction("player hash"))?
        .get(1)
        .unwrap()
        .as_str();

    Ok(format!(
        "https://www.youtube.com/s/player/{}/player_ias.vflset/en_US/base.js",
        player_hash
    ))
}

async fn get_response(http: &Client, url: &str) -> Result<String> {
    let resp = http.get(url).send().await?.error_for_status()?;
    Ok(resp.text().await?)
}

fn get_sts(player_js: &str) -> Result<String> {
    static STS_PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new("signatureTimestamp[=:](\\d+)").unwrap());

    Ok(STS_PATTERN
        .captures(player_js)
        .ok_or(DeobfError::Extraction("sts"))?
        .get(1)
        .unwrap()
        .as_str()
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use path_macro::path;
    use test_log::test;

    static TEST_JS: Lazy<String> = Lazy::new(|| {
        let js_path = path!("testfiles" / "deobf" / "dummy_player.js");
        std::fs::read_to_string(js_path).unwrap()
    });

    const N_DEOBF_FUNC: &str = r#"Vo=function(a){var b=a.split(""),c=[function(d,e,f){var h=f.length;d.forEach(function(l,m,n){this.push(n[m]=f[(f.indexOf(l)-f.indexOf(this[m])+m+h--)%f.length])},e.split(""))},
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
c[36](c[8],c[32]),c[20](c[25],c[10]),c[2](c[22],c[8]),c[32](c[20],c[16]),c[32](c[47],c[49]),c[1](c[44],c[28]),c[39](c[16]),c[32](c[42],c[22]),c[46](c[14],c[48]),c[26](c[29],c[10]),c[46](c[9],c[3]),c[32](c[45])}catch(d){return"enhanced_except_85UBjOr-_w8_"+a}return b.join("")};var deobfuscate=Vo;"#;

    #[test]
    fn t_get_sig_fn_name() {
        let dfunc_name = get_sig_fn_name(&TEST_JS).unwrap();
        assert_eq!(dfunc_name, "Rva");
    }

    #[test]
    fn t_get_sig_fn() {
        let dcode = get_sig_fn(&TEST_JS).unwrap();
        assert_eq!(
            dcode,
            r#"var qB={w8:function(a){a.reverse()},EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},Np:function(a,b){a.splice(0,b)}};var Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};var deobfuscate=Rva;"#
        );
    }

    #[test]
    fn t_deobfuscate_sig() {
        let dcode = get_sig_fn(&TEST_JS).unwrap();
        let deobf = deobfuscate_sig("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i", &dcode).unwrap();
        assert_eq!(deobf, "AOq0QJ8wRAIgaryQHmplJ9xJSKFywyaSMHuuwZYsoMTfvRviG51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5f");
    }

    #[test]
    fn t_get_nsig_fn_name() {
        let name = get_nsig_fn_name(&TEST_JS).unwrap();
        assert_eq!(name, "Vo");
    }

    #[test]
    fn t_extract_js_fn() {
        let base_js = "Wka = function(d){let x=10/2;return /,,[/,913,/](,)}/}let a = 42;";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "Wka = function(d){let x=10/2;return /,,[/,913,/](,)}/}"
        );
    }

    #[test]
    fn t_extract_js_fn_eviljs() {
        let base_js = "Wka = function(d){var x = [/,,/,913,/(,)}/,\"abcdef}\\\"\",];var y = 10/2/1;return x[1][y];}//some={}random-padding+;";
        let res = extract_js_fn(base_js, "Wka").unwrap();
        assert_eq!(
            res,
            "Wka = function(d){var x = [/,,/,913,/(,)}/,\"abcdef}\\\"\",];var y = 10/2/1;return x[1][y];}"
        );
    }

    #[test]
    fn t_get_nsig_fn() {
        let res = get_nsig_fn(&TEST_JS).unwrap();
        assert_eq!(res, N_DEOBF_FUNC);
    }

    #[test]
    fn t_get_sts() {
        let res = get_sts(&TEST_JS).unwrap();
        assert_eq!(res, "19187")
    }

    #[test]
    fn t_deobfuscate_nsig() {
        let res = deobfuscate_nsig("BI_n4PxQ22is-KKajKUW", N_DEOBF_FUNC).unwrap();
        assert_eq!(res, "nrkec0fwgTWolw");
    }

    #[test(tokio::test)]
    async fn t_get_player_js_url() {
        let client = Client::new();
        let url = get_player_js_url(&client).await.unwrap();
        assert!(url.starts_with("https://www.youtube.com/s/player"));
        assert_eq!(url.len(), 73);
    }

    #[test(tokio::test)]
    async fn t_update() {
        let client = Client::new();
        let deobf = Deobfuscator::new(client).await.unwrap();

        let deobf_sig = deobf.deobfuscate_sig("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i").unwrap();
        println!("{}", deobf_sig);
        let deobf_nsig = deobf.deobfuscate_nsig("WHbZ-Nj2TSJxder").unwrap();
        println!("{}", deobf_nsig);
    }
}
