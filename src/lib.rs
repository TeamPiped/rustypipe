use anyhow::{anyhow, bail, Result};
use fancy_regex::Regex;
use once_cell::sync::Lazy;
use quick_js::Context;
use std::result::Result::Ok;

#[macro_use]
mod macros;

const DEOBFUSCATION_FUNC_NAME: &str = "deobfuscate";

fn get_deobfuscation_func_name(player_js: &str) -> Result<String> {
    static FUNCTION_PATTERNS: Lazy<[Regex; 6]> = Lazy::new(|| {
        [
        Regex::new("(?:\\b|[^a-zA-Z0-9$])([a-zA-Z0-9$]{2,})\\s*=\\s*function\\(\\s*a\\s*\\)\\s*\\{\\s*a\\s*=\\s*a\\.split\\(\\s*\"\"\\s*\\)").unwrap(),
        Regex::new("\\bm=([a-zA-Z0-9$]{2,})\\(decodeURIComponent\\(h\\.s\\)\\)").unwrap(),
        Regex::new("\\bc&&\\(c=([a-zA-Z0-9$]{2,})\\(decodeURIComponent\\(c\\)\\)").unwrap(),
        Regex::new("([\\w$]+)\\s*=\\s*function\\((\\w+)\\)\\{\\s*\\2=\\s*\\2\\.split\\(\"\"\\)\\s*;").unwrap(),
        Regex::new("\\b([\\w$]{2,})\\s*=\\s*function\\((\\w+)\\)\\{\\s*\\2=\\s*\\2\\.split\\(\"\"\\)\\s*;").unwrap(),
        Regex::new("\\bc\\s*&&\\s*d\\.set\\([^,]+\\s*,\\s*(:encodeURIComponent\\s*\\()([a-zA-Z0-9$]+)\\(").unwrap(),
    ]
    });

    FUNCTION_PATTERNS
        .iter()
        .find_map(|pattern| pattern.captures(player_js).ok().flatten())
        .map(|c| c.get(1).unwrap().as_str().to_string())
        .ok_or_else(|| anyhow!("could not find deobf function name"))
}

pub fn load_deobfuscation_code(player_js: &str) -> Result<String> {
    let dfunc_name = get_deobfuscation_func_name(player_js)?;

    let function_pattern_str = "(".to_string()
        + &dfunc_name.replace('$', "\\$")
        + "=function\\([a-zA-Z0-9_]+\\)\\{.+?\\})";
    let function_pattern = ok_or_bail!(
        Regex::new(&function_pattern_str),
        Err(anyhow!("could not parse function pattern regex"))
    );

    let deobfuscate_function = "var ".to_string()
        + some_or_bail!(
            function_pattern.captures(player_js).ok().flatten(),
            Err(anyhow!("could not find deobf function"))
        )
        .get(1)
        .unwrap()
        .as_str()
        + ";";

    let helper_object_name_pattern = Regex::new(";([A-Za-z0-9_\\$]{2})\\...\\(").unwrap();
    let helper_object_name = some_or_bail!(
        helper_object_name_pattern
            .captures(&deobfuscate_function)
            .ok()
            .flatten(),
        Err(anyhow!("could not find helper object name"))
    )
    .get(1)
    .unwrap()
    .as_str();

    let helper_pattern_str =
        "(var ".to_string() + &helper_object_name.replace('$', "\\$") + "=\\{.+?\\}\\};)";
    let helper_pattern = ok_or_bail!(
        Regex::new(&helper_pattern_str),
        Err(anyhow!("could not parse helper pattern regex"))
    );
    let player_js_nonl = player_js.replace('\n', "");
    let helper_object = some_or_bail!(
        helper_pattern.captures(&player_js_nonl).ok().flatten(),
        Err(anyhow!("could not find helper object"))
    )
    .get(1)
    .unwrap()
    .as_str();

    let caller_function =
        "function ".to_string() + DEOBFUSCATION_FUNC_NAME + "(a){return " + &dfunc_name + "(a);}";

    Ok(helper_object.to_string() + &deobfuscate_function + &caller_function)
}

pub fn deobfuscate_signature(obfuscated_sig: &str, deobfuscation_code: &str) -> Result<String> {
    let context = Context::new()?;
    context.eval(deobfuscation_code)?;
    let res = context.call_function(DEOBFUSCATION_FUNC_NAME, vec![obfuscated_sig])?;

    match res.as_str() {
        Some(res) => Ok(res.to_string()),
        None => bail!("deobfuscation func returned null"),
    }
}

pub fn get_n_deobfuscation_function_name(player_js: &str) -> Result<String> {
    let function_name_pattern =
        Regex::new("\\.get\\(\"n\"\\)\\)&&\\(b=([a-zA-Z0-9$]+)(?:\\[(\\d+)])?\\([a-zA-Z0-9]\\)")
            .unwrap();

    let fname_match = some_or_bail!(
        function_name_pattern.captures(player_js).ok().flatten(),
        Err(anyhow!("could not find n_deobf function"))
    );

    let function_name = fname_match.get(1).unwrap().as_str();

    if fname_match.len() == 1 {
        return Ok(function_name.to_string());
    }

    let array_num = fname_match.get(2).unwrap().as_str().parse::<u32>()?;
    let array_pattern_str =
        "var ".to_string() + &fancy_regex::escape(function_name) + "\\s*=\\s*\\[(.+?)];";
    let array_pattern = Regex::new(&array_pattern_str)?;
    let array_str = some_or_bail!(
        array_pattern.captures(player_js).ok().flatten(),
        Err(anyhow!("could not find n_deobf array_str"))
    )
    .get(1)
    .unwrap()
    .as_str();
    let mut names = array_str.split(',');
    let name = some_or_bail!(
        names.nth(array_num.try_into()?),
        Err(anyhow!(
            "could not get {}th item from {}",
            array_num,
            array_str
        ))
    );
    Ok(name.to_string())
}

pub fn match_to_closing_parenthesis(string: &str, start: &str) -> Option<String> {
    let mut start_index = string.find(start)?;
    start_index += start.len();

    let mut visited_par = false;
    let mut open_par = 0;
    let mut res = String::new();

    for c in string[start_index..].chars() {
        res.push(c);

        match c {
            '{' => {
                visited_par = true;
                open_par += 1;
            }
            '}' => {
                open_par -= 1;
            }
            _ => {}
        }

        if visited_par && open_par == 0 {
            break;
        }
    }
    Some(res)
}

pub fn parse_n_decode_function(player_js: &str, function_name: &str) -> Result<String> {
    // Find using parentheses
    let function_base = function_name.to_string() + "=function";
    match match_to_closing_parenthesis(player_js, &function_base) {
        Some(m) => Ok(function_base.clone() + &m + ";"),
        None => {
            // Find using regex
            let player_js_nonl = player_js.replace('\n', "");

            let function_pattern_str = function_name.to_string() + "=function(.*?}};)\n";
            let function_pattern = Regex::new(&function_pattern_str)?;
            let function = some_or_bail!(
                function_pattern.captures(&player_js_nonl)?,
                Err(anyhow!("could not find n_decode function"))
            )
            .get(1)
            .unwrap()
            .as_str();

            Ok("function ".to_string() + function)
        }
    }
}

pub fn deobfuscate_n_signature(
    function: &str,
    function_name: &str,
    obfuscated_sig: &str,
) -> Result<String> {
    let context = quick_js::Context::new()?;
    context.eval(function)?;
    let res = context.call_function(function_name, vec![obfuscated_sig])?;

    match res.as_str() {
        Some(res) => Ok(res.to_string()),
        None => bail!("deobfuscation func returned null"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_JS: &str = include_str!("../notes/base.js");
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
c[36](c[8],c[32]),c[20](c[25],c[10]),c[2](c[22],c[8]),c[32](c[20],c[16]),c[32](c[47],c[49]),c[1](c[44],c[28]),c[39](c[16]),c[32](c[42],c[22]),c[46](c[14],c[48]),c[26](c[29],c[10]),c[46](c[9],c[3]),c[32](c[45])}catch(d){return"enhanced_except_85UBjOr-_w8_"+a}return b.join("")};"#;

    #[test]
    fn test_get_deobfuscation_func_name() {
        let dfunc_name = get_deobfuscation_func_name(TEST_JS).unwrap();
        assert_eq!(dfunc_name, "Rva");
    }

    #[test]
    fn test_load_deobfuscation_code() {
        let dcode = load_deobfuscation_code(TEST_JS).unwrap();
        assert_eq!(
            dcode,
            r#"var qB={w8:function(a){a.reverse()},EC:function(a,b){var c=a[0];a[0]=a[b%a.length];a[b%a.length]=c},Np:function(a,b){a.splice(0,b)}};var Rva=function(a){a=a.split("");qB.Np(a,3);qB.w8(a,41);qB.EC(a,55);qB.Np(a,3);qB.w8(a,33);qB.Np(a,3);qB.EC(a,48);qB.EC(a,17);qB.EC(a,43);return a.join("")};function deobfuscate(a){return Rva(a);}"#
        );
    }

    #[test]
    fn test_deobfuscate_signature() {
        let dcode = load_deobfuscation_code(TEST_JS).unwrap();
        let deobf = deobfuscate_signature("GOqGOqGOq0QJ8wRAIgaryQHfplJ9xJSKFywyaSMHuuwZYsoMTAvRvfm51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5fb5i", &dcode).unwrap();
        assert_eq!(deobf, "AOq0QJ8wRAIgaryQHmplJ9xJSKFywyaSMHuuwZYsoMTfvRviG51qIGECIA5061zWeyfMPX9hEl_U6f9J0tr7GTJMKyPf5XNrJb5f");
    }

    #[test]
    fn test_get_n_deobfuscation_function_name() {
        let name = get_n_deobfuscation_function_name(TEST_JS).unwrap();
        assert_eq!(name, "Vo");
    }

    #[test]
    fn test_match_to_closing_parenthesis() {
        let res =
            match_to_closing_parenthesis("Kx Hello { Thx { Bye } } Wut {Tst {}}", "Hello").unwrap();
        assert_eq!(res, " { Thx { Bye } }")
    }

    #[test]
    fn test_parse_n_decode_function() {
        let res = parse_n_decode_function(TEST_JS, "Vo").unwrap();
        assert_eq!(res, N_DEOBF_FUNC);
    }

    #[test]
    fn test_deobfuscate_n_signature() {
        let res = deobfuscate_n_signature(N_DEOBF_FUNC, "Vo", "BI_n4PxQ22is-KKajKUW").unwrap();
        assert_eq!(res, "nrkec0fwgTWolw");
    }
}
